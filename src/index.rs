//! Incremental SQLite index over all session files.
//!
//! Each session file = one row, cached by `(file_mtime, file_size)`. A refresh
//! re-parses only changed/new files (in parallel) and drops rows for files that
//! disappeared, so steady-state refreshes are near-instant.

use crate::model::{Session, Source};
use crate::scan::{self, ScanRoots};
use anyhow::Result;
use rayon::prelude::*;
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};
use std::path::Path;

const SCHEMA_VERSION: i64 = 1;

pub struct Index {
    conn: Connection,
    roots: ScanRoots,
}

pub struct RefreshStats {
    pub parsed: usize,
    pub deleted: usize,
    pub total: usize,
}

impl Index {
    /// Open (creating if needed) the index at the default location
    /// (`~/.termem/index.db`).
    pub fn open_default() -> Result<Index> {
        let dir = scan::home().join(".termem");
        std::fs::create_dir_all(&dir)?;
        Index::open(&dir.join("index.db"))
    }

    /// Open with the standard `$HOME` session locations.
    pub fn open(path: &Path) -> Result<Index> {
        Index::open_with_roots(path, ScanRoots::home())
    }

    /// Open with explicit scan roots (used for custom locations and tests).
    pub fn open_with_roots(path: &Path, roots: ScanRoots) -> Result<Index> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let mut index = Index { conn, roots };
        index.ensure_schema()?;
        Ok(index)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    fn ensure_schema(&mut self) -> Result<()> {
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap_or(0);
        if version != SCHEMA_VERSION {
            self.conn.execute_batch("DROP TABLE IF EXISTS sessions;")?;
        }
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                file_path   TEXT PRIMARY KEY,
                id          TEXT NOT NULL,
                source      TEXT NOT NULL,
                cwd         TEXT NOT NULL,
                title       TEXT NOT NULL,
                first_prompt TEXT NOT NULL,
                last_prompt TEXT NOT NULL,
                model       TEXT,
                git_branch  TEXT,
                started_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL,
                msg_count   INTEGER NOT NULL,
                file_mtime  INTEGER NOT NULL,
                file_size   INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_sessions_cwd ON sessions(cwd);
            CREATE INDEX IF NOT EXISTS idx_sessions_updated ON sessions(updated_at);
            CREATE INDEX IF NOT EXISTS idx_sessions_id ON sessions(id);",
        )?;
        self.conn
            .pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(())
    }

    fn load_existing(&self) -> Result<HashMap<String, (i64, i64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT file_path, file_mtime, file_size FROM sessions")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, (r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let (k, v) = row?;
            map.insert(k, v);
        }
        Ok(map)
    }

    /// Bring the index up to date with the filesystem.
    pub fn refresh(&mut self) -> Result<RefreshStats> {
        let candidates = scan::gather_candidates(&self.roots);
        let existing = self.load_existing()?;
        let present: HashSet<String> = candidates
            .iter()
            .map(|c| c.path.to_string_lossy().to_string())
            .collect();
        let thread_map = scan::codex::load_thread_map(self.roots.codex.as_deref());

        // Parse only changed/new files, in parallel.
        let parsed: Vec<(Session, i64, i64)> = candidates
            .par_iter()
            .filter_map(|c| {
                let key = c.path.to_string_lossy();
                let changed = match existing.get(key.as_ref()) {
                    Some((mt, sz)) => *mt != c.mtime_ms || *sz != c.size,
                    None => true,
                };
                if !changed {
                    return None;
                }
                scan::parse_candidate(c, &thread_map)
            })
            .collect();

        let to_delete: Vec<String> = existing
            .keys()
            .filter(|p| !present.contains(*p))
            .cloned()
            .collect();

        let tx = self.conn.transaction()?;
        for path in &to_delete {
            tx.execute("DELETE FROM sessions WHERE file_path = ?1", params![path])?;
        }
        for (s, mtime, size) in &parsed {
            upsert(&tx, s, *mtime, *size)?;
        }
        tx.commit()?;

        Ok(RefreshStats {
            parsed: parsed.len(),
            deleted: to_delete.len(),
            total: candidates.len(),
        })
    }
}

fn upsert(conn: &Connection, s: &Session, mtime: i64, size: i64) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO sessions
            (file_path, id, source, cwd, title, first_prompt, last_prompt,
             model, git_branch, started_at, updated_at, msg_count, file_mtime, file_size)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
        params![
            s.file_path,
            s.id,
            s.source.as_str(),
            s.cwd,
            s.title,
            s.first_prompt,
            s.last_prompt,
            s.model,
            s.git_branch,
            s.started_at,
            s.updated_at,
            s.msg_count,
            mtime,
            size,
        ],
    )?;
    Ok(())
}

/// Map a query row (column order matches [`query`](crate::query)) to a Session.
pub fn row_to_session(r: &rusqlite::Row) -> rusqlite::Result<Session> {
    let source_str: String = r.get("source")?;
    Ok(Session {
        file_path: r.get("file_path")?,
        id: r.get("id")?,
        source: Source::from_str(&source_str).unwrap_or(Source::Shell),
        cwd: r.get("cwd")?,
        title: r.get("title")?,
        first_prompt: r.get("first_prompt")?,
        last_prompt: r.get("last_prompt")?,
        model: r.get("model")?,
        git_branch: r.get("git_branch")?,
        started_at: r.get("started_at")?,
        updated_at: r.get("updated_at")?,
        msg_count: r.get("msg_count")?,
    })
}
