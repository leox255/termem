//! Agent-authored summaries: the shared-memory half of termem.
//!
//! Summaries live in termem's own `summaries` table (never in the source
//! session files). `recall` joins sessions with their summaries and reports a
//! freshness state so an agent knows when to (re)distil one.

use crate::model::{Session, Source};
use crate::query::{escape_like, scope_clause, Scope};
use anyhow::Result;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryState {
    /// A summary exists and the source has not changed since it was written.
    Cached,
    /// No summary yet; an agent should read the session and write one.
    NeedsSummary,
    /// A summary exists but the source grew/changed since; refresh it.
    Stale,
}

impl SummaryState {
    pub fn as_str(self) -> &'static str {
        match self {
            SummaryState::Cached => "cached",
            SummaryState::NeedsSummary => "needs_summary",
            SummaryState::Stale => "stale",
        }
    }
}

pub struct RecallRow {
    pub session: Session,
    pub summary: Option<String>,
    pub unfinished: Option<String>,
    pub state: SummaryState,
}

/// Store (or replace) an agent-written summary. Returns `true` if the session
/// is known to the index (so staleness can be tracked).
pub fn save_summary(
    conn: &Connection,
    id: &str,
    summary: &str,
    unfinished: Option<&str>,
    tags: &[String],
) -> Result<bool> {
    let stat: Option<(i64, i64, i64)> = conn
        .query_row(
            "SELECT file_mtime, file_size, updated_at FROM sessions WHERE id = ?1 LIMIT 1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (mtime, size, src_updated) = stat.unwrap_or((0, 0, 0));
    let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO summaries
            (session_id, summary, unfinished, tags, source_mtime, source_size, source_updated, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?8)
         ON CONFLICT(session_id) DO UPDATE SET
            summary=excluded.summary, unfinished=excluded.unfinished, tags=excluded.tags,
            source_mtime=excluded.source_mtime, source_size=excluded.source_size,
            source_updated=excluded.source_updated, updated_at=excluded.updated_at",
        rusqlite::params![id, summary, unfinished, tags_json, mtime, size, src_updated, now],
    )?;
    Ok(stat.is_some())
}

pub fn has_summary(conn: &Connection, id: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM summaries WHERE session_id = ?1",
        [id],
        |_| Ok(()),
    )
    .optional()
    .ok()
    .flatten()
    .is_some()
}

/// Sessions in scope joined with their summaries and freshness state, newest
/// first.
pub fn recall(
    conn: &Connection,
    cwd: &str,
    scope: Scope,
    search: Option<&str>,
    limit: i64,
) -> Result<Vec<RecallRow>> {
    let (scope_sql, mut args) = scope_clause(scope, cwd);
    // `summaries` has no cwd/title columns, so the unprefixed columns added by
    // scope_clause resolve unambiguously to `sessions`.
    let mut sql = String::from(
        "SELECT s.file_path, s.id, s.source, s.cwd, s.title, s.first_prompt, s.last_prompt, \
         s.model, s.git_branch, s.started_at, s.updated_at, s.msg_count, \
         sm.summary, sm.unfinished, sm.source_updated \
         FROM sessions s LEFT JOIN summaries sm ON sm.session_id = s.id WHERE 1=1",
    );
    sql.push_str(&scope_sql);
    if let Some(q) = search {
        let q = q.trim();
        if !q.is_empty() {
            sql.push_str(
                " AND (s.title LIKE ? ESCAPE '\\' OR s.first_prompt LIKE ? ESCAPE '\\' \
                 OR s.cwd LIKE ? ESCAPE '\\')",
            );
            let pat = format!("%{}%", escape_like(q));
            for _ in 0..3 {
                args.push(Value::Text(pat.clone()));
            }
        }
    }
    sql.push_str(" ORDER BY s.updated_at DESC LIMIT ?");
    args.push(Value::Integer(limit.max(1)));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
        let session = Session {
            file_path: r.get(0)?,
            id: r.get(1)?,
            source: Source::from_tag(&r.get::<_, String>(2)?).unwrap_or(Source::Shell),
            cwd: r.get(3)?,
            title: r.get(4)?,
            first_prompt: r.get(5)?,
            last_prompt: r.get(6)?,
            model: r.get(7)?,
            git_branch: r.get(8)?,
            started_at: r.get(9)?,
            updated_at: r.get(10)?,
            msg_count: r.get(11)?,
        };
        let summary: Option<String> = r.get(12)?;
        let unfinished: Option<String> = r.get(13)?;
        let src_updated: Option<i64> = r.get(14)?;
        // Per-session staleness: the session's own last-active time, which is
        // correct even when many sessions share one file (Gemini, opencode).
        let state = match summary {
            None => SummaryState::NeedsSummary,
            Some(_) if src_updated == Some(session.updated_at) => SummaryState::Cached,
            Some(_) => SummaryState::Stale,
        };
        Ok(RecallRow {
            session,
            summary,
            unfinished,
            state,
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
    use crate::query::Scope;
    use crate::scan::ScanRoots;
    use std::sync::atomic::{AtomicU64, Ordering};

    static N: AtomicU64 = AtomicU64::new(0);

    fn empty_index() -> Index {
        // Unique per call so parallel tests never share a SQLite file.
        let n = N.fetch_add(1, Ordering::Relaxed);
        let db = std::env::temp_dir().join(format!("termem-mem-{}-{}.db", std::process::id(), n));
        let _ = std::fs::remove_file(&db);
        // No real roots: an empty session cache, just the summaries table.
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
    fn save_and_read_summary() {
        let idx = empty_index();
        let conn = idx.conn();
        // Unknown session id (not in cache) still saves, returns false.
        let known = save_summary(conn, "sid", "did the thing", Some("ran nothing"), &[]).unwrap();
        assert!(!known);
        assert!(has_summary(conn, "sid"));
        assert!(!has_summary(conn, "other"));
    }

    #[test]
    fn staleness_follows_session_updated_at() {
        let idx = empty_index();
        let conn = idx.conn();
        conn.execute(
            "INSERT INTO sessions
                (key, file_path, id, source, cwd, title, first_prompt, last_prompt,
                 model, git_branch, started_at, updated_at, msg_count, file_mtime, file_size)
             VALUES ('k','/f','s1','claude','/work','T','','',NULL,NULL,0,100,1,0,0)",
            [],
        )
        .unwrap();

        // No summary yet.
        let rows = recall(conn, "/work", Scope::Subtree, None, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].state, SummaryState::NeedsSummary);

        // Summary written -> cached.
        save_summary(conn, "s1", "did it", None, &[]).unwrap();
        let rows = recall(conn, "/work", Scope::Subtree, None, 10).unwrap();
        assert_eq!(rows[0].state, SummaryState::Cached);

        // Session gains activity (updated_at moves) -> stale.
        conn.execute("UPDATE sessions SET updated_at = 200 WHERE id = 's1'", [])
            .unwrap();
        let rows = recall(conn, "/work", Scope::Subtree, None, 10).unwrap();
        assert_eq!(rows[0].state, SummaryState::Stale);
    }
}
