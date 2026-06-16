//! Directory-scoped queries over the index.

use crate::index::row_to_session;
use crate::model::{Session, Source};
use anyhow::Result;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Sessions started exactly in `cwd`.
    Here,
    /// Sessions started in `cwd` or any descendant directory.
    Subtree,
    /// Every session, ignoring `cwd`.
    All,
}

const COLUMNS: &str = "file_path, id, source, cwd, title, first_prompt, last_prompt, \
     model, git_branch, started_at, updated_at, msg_count, bypass";

/// Escape SQL LIKE wildcards so the text matches literally (use with `ESCAPE '\\'`).
pub(crate) fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '%' || c == '_' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// `<cwd><sep>%` pattern matching descendants of `cwd`. The separator is `/`
/// on Unix and `\` on Windows, escaped so it matches literally under
/// `ESCAPE '\\'`.
fn like_prefix(cwd: &str) -> String {
    let sep = std::path::MAIN_SEPARATOR.to_string();
    format!("{}{}%", escape_like(cwd), escape_like(&sep))
}

/// SQL fragment + bound args restricting rows to a directory scope.
pub(crate) fn scope_clause(scope: Scope, cwd: &str) -> (String, Vec<Value>) {
    match scope {
        Scope::Here => (
            " AND cwd = ?".to_string(),
            vec![Value::Text(cwd.to_string())],
        ),
        Scope::Subtree => (
            " AND (cwd = ? OR cwd LIKE ? ESCAPE '\\')".to_string(),
            vec![Value::Text(cwd.to_string()), Value::Text(like_prefix(cwd))],
        ),
        Scope::All => (String::new(), Vec::new()),
    }
}

pub fn query(
    conn: &Connection,
    cwd: &str,
    scope: Scope,
    sources: &[Source],
    search: Option<&str>,
    limit: i64,
) -> Result<Vec<Session>> {
    let (scope_sql, mut args) = scope_clause(scope, cwd);
    let mut sql = format!("SELECT {COLUMNS} FROM sessions WHERE 1=1{scope_sql}");

    if !sources.is_empty() {
        let placeholders = sources.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        sql.push_str(&format!(" AND source IN ({placeholders})"));
        for s in sources {
            args.push(Value::Text(s.as_str().to_string()));
        }
    }

    if let Some(q) = search {
        let q = q.trim();
        if !q.is_empty() {
            sql.push_str(
                " AND (title LIKE ? ESCAPE '\\' OR first_prompt LIKE ? ESCAPE '\\' \
                 OR cwd LIKE ? ESCAPE '\\' OR id LIKE ? ESCAPE '\\')",
            );
            let pat = format!("%{}%", escape_like(q));
            for _ in 0..4 {
                args.push(Value::Text(pat.clone()));
            }
        }
    }

    sql.push_str(" ORDER BY updated_at DESC LIMIT ?");
    args.push(Value::Integer(limit.max(1)));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), row_to_session)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Per-source counts in a directory scope, as `(source_tag, count)` pairs. Uses
/// a COUNT aggregate so it stays cheap and correct no matter how many sessions
/// match (the cd hint calls this on every directory change).
pub fn counts_by_source(conn: &Connection, cwd: &str, scope: Scope) -> Result<Vec<(String, i64)>> {
    let (scope_sql, args) = scope_clause(scope, cwd);
    let sql = format!("SELECT source, COUNT(*) FROM sessions WHERE 1=1{scope_sql} GROUP BY source");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Resolve a session for `resume`: exact id wins, then id prefix, then fuzzy
/// text, most-recent first.
pub fn find_one(conn: &Connection, needle: &str) -> Result<Option<Session>> {
    if let Some(s) = query_by_id(conn, needle, true)? {
        return Ok(Some(s));
    }
    if let Some(s) = query_by_id(conn, needle, false)? {
        return Ok(Some(s));
    }
    let fuzzy = query(conn, "", Scope::All, &[], Some(needle), 1)?;
    Ok(fuzzy.into_iter().next())
}

fn query_by_id(conn: &Connection, id: &str, exact: bool) -> Result<Option<Session>> {
    let sql = format!(
        "SELECT {COLUMNS} FROM sessions WHERE id {} ORDER BY updated_at DESC LIMIT 1",
        if exact { "= ?" } else { "LIKE ? ESCAPE '\\'" }
    );
    let pat = if exact {
        id.to_string()
    } else {
        format!("{}%", escape_like(id))
    };
    let mut stmt = conn.prepare(&sql)?;
    let found = stmt
        .query_row(rusqlite::params![pat], row_to_session)
        .optional()?;
    Ok(found)
}

/// Combined search: message-content matches (FTS) first, then metadata matches
/// (title / first prompt / path / id), deduped, capped at `limit`.
pub fn search(
    conn: &Connection,
    query_s: &str,
    cwd: &str,
    scope: Scope,
    sources: &[Source],
    limit: i64,
) -> Result<Vec<Session>> {
    let q = query_s.trim();
    let cap = limit.max(1) as usize;
    let mut out: Vec<Session> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // 1) Content matches via FTS5 (best-effort: degrade to metadata on any error).
    if !q.is_empty() {
        if let Some(matchq) = fts_query(q) {
            if let Ok(rows) = fts_search(conn, &matchq, cwd, scope, sources, limit) {
                for s in rows {
                    if out.len() >= cap {
                        break;
                    }
                    if seen.insert(s.id.clone()) {
                        out.push(s);
                    }
                }
            }
        }
    }

    // 2) Metadata matches fill the remaining slots.
    let meta = query(
        conn,
        cwd,
        scope,
        sources,
        if q.is_empty() { None } else { Some(q) },
        limit,
    )?;
    for s in meta {
        if out.len() >= cap {
            break;
        }
        if seen.insert(s.id.clone()) {
            out.push(s);
        }
    }
    Ok(out)
}

/// Turn free text into a safe FTS5 MATCH string: each alphanumeric token quoted
/// (so FTS operators can't be injected) and AND-ed together.
fn fts_query(q: &str) -> Option<String> {
    let tokens: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\""))
        .collect();
    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" "))
    }
}

fn fts_search(
    conn: &Connection,
    matchq: &str,
    cwd: &str,
    scope: Scope,
    sources: &[Source],
    limit: i64,
) -> Result<Vec<Session>> {
    let (scope_sql, scope_args) = scope_clause(scope, cwd);
    let mut args: Vec<Value> = vec![Value::Text(matchq.to_string())];
    args.extend(scope_args);
    let mut sql = format!(
        "SELECT s.file_path, s.id, s.source, s.cwd, s.title, s.first_prompt, s.last_prompt, \
         s.model, s.git_branch, s.started_at, s.updated_at, s.msg_count, s.bypass \
         FROM content_fts JOIN sessions s ON s.id = content_fts.session_id \
         WHERE content_fts MATCH ?{scope_sql}"
    );
    if !sources.is_empty() {
        let placeholders = sources.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        sql.push_str(&format!(" AND s.source IN ({placeholders})"));
        for s in sources {
            args.push(Value::Text(s.as_str().to_string()));
        }
    }
    sql.push_str(" ORDER BY rank LIMIT ?");
    args.push(Value::Integer(limit.max(1)));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), row_to_session)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}
