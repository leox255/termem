//! Directory-scoped queries over the index.

use crate::index::row_to_session;
use crate::model::{Session, Source};
use anyhow::Result;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension};

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
     model, git_branch, started_at, updated_at, msg_count";

/// Escape SQL LIKE wildcards so the text matches literally (use with `ESCAPE '\\'`).
fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '%' || c == '_' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// `<cwd>/%` pattern matching descendants of `cwd`.
fn like_prefix(cwd: &str) -> String {
    format!("{}/%", escape_like(cwd))
}

/// SQL fragment + bound args restricting rows to a directory scope.
fn scope_clause(scope: Scope, cwd: &str) -> (String, Vec<Value>) {
    match scope {
        Scope::Here => (" AND cwd = ?".to_string(), vec![Value::Text(cwd.to_string())]),
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

    if !sources.is_empty() && sources.len() < 3 {
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

/// Exact per-source counts in a directory scope, as `(claude, codex, shell)`.
/// Uses a COUNT aggregate so it stays cheap and correct no matter how many
/// sessions match (the cd hint calls this on every directory change).
pub fn counts_by_source(conn: &Connection, cwd: &str, scope: Scope) -> Result<(i64, i64, i64)> {
    let (scope_sql, args) = scope_clause(scope, cwd);
    let sql = format!("SELECT source, COUNT(*) FROM sessions WHERE 1=1{scope_sql} GROUP BY source");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    let (mut claude, mut codex, mut shell) = (0i64, 0i64, 0i64);
    for row in rows {
        let (src, n) = row?;
        match src.as_str() {
            "claude" => claude = n,
            "codex" => codex = n,
            "shell" => shell = n,
            _ => {}
        }
    }
    Ok((claude, codex, shell))
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
