//! Directory-scoped queries over the index.

use crate::index::row_to_session;
use crate::model::{Session, Source};
use anyhow::Result;
use rusqlite::types::Value;
use rusqlite::Connection;

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

/// Build a `LIKE` prefix pattern (`<cwd>/%`) with SQL wildcards escaped.
fn like_prefix(cwd: &str) -> String {
    let mut s = String::with_capacity(cwd.len() + 2);
    for c in cwd.chars() {
        if c == '%' || c == '_' || c == '\\' {
            s.push('\\');
        }
        s.push(c);
    }
    s.push('/');
    s.push('%');
    s
}

pub fn query(
    conn: &Connection,
    cwd: &str,
    scope: Scope,
    sources: &[Source],
    search: Option<&str>,
    limit: i64,
) -> Result<Vec<Session>> {
    let mut sql = format!("SELECT {COLUMNS} FROM sessions WHERE 1=1");
    let mut args: Vec<Value> = Vec::new();

    match scope {
        Scope::Here => {
            sql.push_str(" AND cwd = ?");
            args.push(Value::Text(cwd.to_string()));
        }
        Scope::Subtree => {
            sql.push_str(" AND (cwd = ? OR cwd LIKE ? ESCAPE '\\')");
            args.push(Value::Text(cwd.to_string()));
            args.push(Value::Text(like_prefix(cwd)));
        }
        Scope::All => {}
    }

    if !sources.is_empty() && sources.len() < 3 {
        let placeholders = sources.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        sql.push_str(&format!(" AND source IN ({placeholders})"));
        for s in sources {
            args.push(Value::Text(s.as_str().to_string()));
        }
    }

    if let Some(q) = search {
        if !q.trim().is_empty() {
            sql.push_str(
                " AND (title LIKE ? OR first_prompt LIKE ? OR cwd LIKE ? OR id LIKE ?)",
            );
            let pat = format!("%{}%", q.trim());
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

/// Find a single session by id prefix or fuzzy text, most-recent first.
pub fn find_one(conn: &Connection, needle: &str) -> Result<Option<Session>> {
    // Exact id first.
    let exact = query(
        conn,
        "",
        Scope::All,
        &[],
        Some(needle),
        1,
    )?;
    Ok(exact.into_iter().next())
}
