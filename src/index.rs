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

const SCHEMA_VERSION: i64 = 2;

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
        // `key` is the primary key (one file can yield several sessions, e.g. a
        // shell log split per directory); `file_path` is the cache unit.
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                key         TEXT PRIMARY KEY,
                file_path   TEXT NOT NULL,
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
            CREATE INDEX IF NOT EXISTS idx_sessions_file ON sessions(file_path);
            CREATE INDEX IF NOT EXISTS idx_sessions_cwd ON sessions(cwd);
            CREATE INDEX IF NOT EXISTS idx_sessions_updated ON sessions(updated_at);
            CREATE INDEX IF NOT EXISTS idx_sessions_id ON sessions(id);",
        )?;
        self.conn
            .pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(())
    }

    /// Map each cached `file_path` to its stat info (rows from one file share it).
    fn load_existing(&self) -> Result<HashMap<String, (i64, i64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT file_path, file_mtime, file_size FROM sessions GROUP BY file_path")?;
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

        // Files whose (mtime, size) changed since they were last indexed.
        let changed: Vec<&scan::Candidate> = candidates
            .iter()
            .filter(|c| {
                let key = c.path.to_string_lossy();
                match existing.get(key.as_ref()) {
                    Some((mt, sz)) => *mt != c.mtime_ms || *sz != c.size,
                    None => true,
                }
            })
            .collect();

        // Re-parse changed files in parallel (a file may yield several rows).
        let parsed: Vec<(Session, i64, i64)> = changed
            .par_iter()
            .flat_map(|&c| scan::parse_candidate(c, &thread_map))
            .collect();

        let to_delete: Vec<String> = existing
            .keys()
            .filter(|p| !present.contains(*p))
            .cloned()
            .collect();

        let tx = self.conn.transaction()?;
        // Clear all rows for changed files first so stale per-directory rows
        // (e.g. a shell log that no longer touches a directory) don't linger.
        for c in &changed {
            tx.execute(
                "DELETE FROM sessions WHERE file_path = ?1",
                params![c.path.to_string_lossy().to_string()],
            )?;
        }
        for (s, mtime, size) in &parsed {
            upsert(&tx, s, *mtime, *size)?;
        }
        for path in &to_delete {
            tx.execute("DELETE FROM sessions WHERE file_path = ?1", params![path])?;
        }
        tx.commit()?;

        Ok(RefreshStats {
            parsed: parsed.len(),
            deleted: to_delete.len(),
            total: candidates.len(),
        })
    }
}

/// Unique row key. Shell logs split into one row per directory, so they include
/// the directory; Claude/Codex are one row per file.
fn session_key(s: &Session) -> String {
    match s.source {
        Source::Shell => format!("{}#{}", s.file_path, s.cwd),
        _ => s.file_path.clone(),
    }
}

fn upsert(conn: &Connection, s: &Session, mtime: i64, size: i64) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO sessions
            (key, file_path, id, source, cwd, title, first_prompt, last_prompt,
             model, git_branch, started_at, updated_at, msg_count, file_mtime, file_size)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        params![
            session_key(s),
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
