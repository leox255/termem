//! Reader for opencode's SQLite session store.
//!
//! `~/.local/share/opencode/opencode.db` has a `session` table with `id`,
//! `directory` (cwd), `title`, and `time_created` / `time_updated` (epoch ms),
//! plus a `message` table for per-session counts. Top-level sessions have a
//! null `parent_id`; sub-sessions (agents/forks) are skipped. opencode resumes
//! by id (`opencode --session <id>`).

use crate::model::{Session, Source};
use anyhow::Result;
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

pub fn parse(path: &Path) -> Result<Vec<Session>> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.busy_timeout(std::time::Duration::from_secs(3))?;
    read_sessions(&conn, path.to_string_lossy().as_ref())
}

pub fn read_sessions(conn: &Connection, file_path: &str) -> Result<Vec<Session>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.directory, s.title, s.time_created, s.time_updated,
                (SELECT COUNT(*) FROM message m WHERE m.session_id = s.id)
         FROM session s
         WHERE s.parent_id IS NULL
         ORDER BY s.time_updated DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        let title: String = r.get(2)?;
        let title = if title.trim().is_empty() {
            "(opencode session)".to_string()
        } else {
            title
        };
        Ok(Session {
            id: r.get::<_, String>(0)?,
            source: Source::Opencode,
            file_path: file_path.to_string(),
            cwd: r.get::<_, String>(1)?,
            title,
            first_prompt: String::new(),
            last_prompt: String::new(),
            model: None,
            git_branch: None,
            started_at: r.get::<_, i64>(3)?,
            updated_at: r.get::<_, i64>(4)?,
            msg_count: r.get::<_, i64>(5)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        let s = row?;
        if !s.cwd.trim().is_empty() {
            out.push(s);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE session (
                id TEXT PRIMARY KEY, project_id TEXT, parent_id TEXT,
                directory TEXT NOT NULL, title TEXT NOT NULL,
                time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL
             );
             CREATE TABLE message (
                id TEXT PRIMARY KEY, session_id TEXT NOT NULL
             );
             INSERT INTO session VALUES ('ses_1','p',NULL,'/work/a','Build a thing',1000,2000);
             INSERT INTO session VALUES ('ses_2','p','ses_1','/work/a','sub agent',1100,1200);
             INSERT INTO message VALUES ('m1','ses_1');
             INSERT INTO message VALUES ('m2','ses_1');",
        )
        .unwrap();
        conn
    }

    #[test]
    fn reads_top_level_sessions_with_counts() {
        let conn = fixture_db();
        let sessions = read_sessions(&conn, "/db/opencode.db").unwrap();
        assert_eq!(
            sessions.len(),
            1,
            "sub-sessions (parent_id set) are skipped"
        );
        let s = &sessions[0];
        assert_eq!(s.id, "ses_1");
        assert_eq!(s.cwd, "/work/a");
        assert_eq!(s.title, "Build a thing");
        assert_eq!(s.source, Source::Opencode);
        assert_eq!(s.started_at, 1000);
        assert_eq!(s.updated_at, 2000);
        assert_eq!(s.msg_count, 2);
    }
}
