//! A per-directory message board: agents post short notes scoped to a working
//! directory; other sessions in the same directory tree read them. Like
//! `summaries`, board posts live only in termem's own store, never in the
//! source session files.
//!
//! This is a *pull* medium. A session sees posts when it calls `read` (e.g. on
//! entering a directory), not before — termem is an MCP server and cannot wake
//! an idle agent. Use it for shared async state ("I'm refactoring auth, don't
//! touch it", "migration is written but unrun"), not live chat.

use crate::query::{scope_clause, Scope};
use anyhow::Result;
use rusqlite::types::Value;
use rusqlite::Connection;

pub struct Post {
    pub id: i64,
    pub cwd: String,
    pub author: Option<String>,
    pub kind: String,
    pub body: String,
    pub created_at: i64,
}

/// Append a post to the board for `cwd`. Returns the new post's id.
pub fn post(
    conn: &Connection,
    cwd: &str,
    author: Option<&str>,
    kind: &str,
    body: &str,
) -> Result<i64> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO board (cwd, author, kind, body, created_at) VALUES (?1,?2,?3,?4,?5)",
        rusqlite::params![cwd, author, kind, body, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Posts visible from `cwd` under `scope`, newest first. `since` keeps only
/// posts created strictly after that epoch-ms cursor (0 = all). `kind` filters
/// by kind when set.
pub fn read(
    conn: &Connection,
    cwd: &str,
    scope: Scope,
    since: i64,
    kind: Option<&str>,
    limit: i64,
) -> Result<Vec<Post>> {
    // `board` names its directory column `cwd`, so scope_clause (which emits
    // `cwd = ?` / `cwd LIKE ?`) applies unchanged and matches session scoping.
    let (scope_sql, mut args) = scope_clause(scope, cwd);
    let mut sql = String::from(
        "SELECT id, cwd, author, kind, body, created_at FROM board WHERE created_at > ?",
    );
    args.insert(0, Value::Integer(since));
    sql.push_str(&scope_sql);
    if let Some(k) = kind {
        let k = k.trim();
        if !k.is_empty() {
            sql.push_str(" AND kind = ?");
            args.push(Value::Text(k.to_string()));
        }
    }
    sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ?");
    args.push(Value::Integer(limit.max(1)));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
        Ok(Post {
            id: r.get(0)?,
            cwd: r.get(1)?,
            author: r.get(2)?,
            kind: r.get(3)?,
            body: r.get(4)?,
            created_at: r.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::Index;
    use crate::scan::ScanRoots;
    use std::sync::atomic::{AtomicU64, Ordering};

    static N: AtomicU64 = AtomicU64::new(0);

    fn empty_index() -> Index {
        let n = N.fetch_add(1, Ordering::Relaxed);
        let db = std::env::temp_dir().join(format!("termem-board-{}-{}.db", std::process::id(), n));
        let _ = std::fs::remove_file(&db);
        Index::open_with_roots(
            &db,
            ScanRoots {
                claude: None,
                codex: None,
                gemini: None,
                opencode: None,
                shell: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn post_and_read_round_trip() {
        let idx = empty_index();
        let conn = idx.conn();
        post(conn, "/work", Some("alice"), "note", "hello").unwrap();
        let posts = read(conn, "/work", Scope::Here, 0, None, 10).unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].body, "hello");
        assert_eq!(posts[0].author.as_deref(), Some("alice"));
        assert_eq!(posts[0].kind, "note");
    }

    #[test]
    fn scope_filters_by_directory() {
        let idx = empty_index();
        let conn = idx.conn();
        post(conn, "/work", None, "note", "root").unwrap();
        post(conn, "/work/sub", None, "note", "child").unwrap();
        post(conn, "/other", None, "note", "elsewhere").unwrap();

        // Here = exact dir only.
        let here = read(conn, "/work", Scope::Here, 0, None, 10).unwrap();
        assert_eq!(here.len(), 1);
        assert_eq!(here[0].body, "root");

        // Subtree = dir + descendants, but not siblings.
        let tree = read(conn, "/work", Scope::Subtree, 0, None, 10).unwrap();
        let bodies: Vec<&str> = tree.iter().map(|p| p.body.as_str()).collect();
        assert_eq!(tree.len(), 2);
        assert!(bodies.contains(&"root"));
        assert!(bodies.contains(&"child"));
        assert!(!bodies.contains(&"elsewhere"));
    }

    #[test]
    fn since_cursor_returns_only_newer_posts() {
        let idx = empty_index();
        let conn = idx.conn();
        let _id1 = post(conn, "/work", None, "note", "first").unwrap();
        let all = read(conn, "/work", Scope::Here, 0, None, 10).unwrap();
        let cursor = all[0].created_at;
        // Nothing strictly after the latest post yet.
        let none = read(conn, "/work", Scope::Here, cursor, None, 10).unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn kind_filter() {
        let idx = empty_index();
        let conn = idx.conn();
        post(conn, "/work", None, "note", "n").unwrap();
        post(conn, "/work", None, "claim", "c").unwrap();
        let claims = read(conn, "/work", Scope::Here, 0, Some("claim"), 10).unwrap();
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].body, "c");
    }
}
