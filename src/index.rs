//! Incremental SQLite index over all session files.
//!
//! Each session file = one row, cached by `(file_mtime, file_size)`. A refresh
//! re-parses only changed/new files (in parallel) and drops rows for files that
//! disappeared, so steady-state refreshes are near-instant.

use crate::model::{Session, Source};
use crate::scan::{self, ScanRoots};
use crate::transcript;
use anyhow::Result;
use rayon::prelude::*;
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};
use std::path::Path;

const SCHEMA_VERSION: i64 = 3;

/// How much message text to fold into the content search index per session.
const FTS_MSG_CAP: usize = 400;
const FTS_BODY_MAX_CHARS: usize = 200_000;

/// A parsed session, its file stat (mtime, size), and its searchable body.
type ScannedBody = (Session, i64, i64, String);

/// Index DB path: `$TERMEM_DB` if set, else `~/.termem/index.db`.
fn db_path() -> Result<std::path::PathBuf> {
    if let Ok(p) = std::env::var("TERMEM_DB") {
        let p = std::path::PathBuf::from(p);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        return Ok(p);
    }
    let dir = scan::home().join(".termem");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("index.db"))
}

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
        Index::open_with_roots(&db_path()?, ScanRoots::from_env())
    }

    /// Open the existing index for a read-only query WITHOUT touching the schema
    /// (no migration, no writes). The cd hint uses this so a passive `cd` never
    /// rebuilds or mutates the cache.
    pub fn open_cached() -> Result<Index> {
        let conn = Connection::open(db_path()?)?;
        conn.busy_timeout(std::time::Duration::from_secs(3))?;
        Ok(Index {
            conn,
            roots: ScanRoots::from_env(),
        })
    }

    /// Open with the standard `$HOME` session locations.
    pub fn open(path: &Path) -> Result<Index> {
        Index::open_with_roots(path, ScanRoots::home())
    }

    /// Open with explicit scan roots (used for custom locations and tests).
    pub fn open_with_roots(path: &Path, roots: ScanRoots) -> Result<Index> {
        let conn = Connection::open(path)?;
        // Set the busy timeout first so even the journal/sync pragmas wait on a
        // transient lock instead of erroring under refresh contention.
        conn.busy_timeout(std::time::Duration::from_secs(3))?;
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
        // Agent-authored summaries: the durable, valuable half of the store.
        // Created unconditionally and never dropped when the session cache is
        // rebuilt for a new SCHEMA_VERSION.
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS summaries (
                session_id   TEXT PRIMARY KEY,
                summary      TEXT NOT NULL,
                unfinished   TEXT,
                tags         TEXT,
                source_mtime INTEGER,
                source_size  INTEGER,
                created_at   INTEGER,
                updated_at   INTEGER
            );",
        )?;
        // Per-session staleness signal (0.4.0). Best-effort ALTER so an existing
        // store upgrades in place without dropping its summaries.
        let _ = self.conn.execute(
            "ALTER TABLE summaries ADD COLUMN source_updated INTEGER",
            [],
        );

        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap_or(0);
        if version == SCHEMA_VERSION {
            return Ok(());
        }
        // Fresh or outdated DB: (re)create the schema and stamp the version.
        // `key` is the primary key (one file can yield several sessions, e.g. a
        // shell log split per directory); `file_path` is the cache unit.
        self.conn.execute_batch(
            "DROP TABLE IF EXISTS sessions;
            CREATE TABLE sessions (
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
            CREATE INDEX IF NOT EXISTS idx_sessions_id ON sessions(id);
            DROP TABLE IF EXISTS content_fts;
            CREATE VIRTUAL TABLE content_fts USING fts5(
                session_id UNINDEXED, body, tokenize = 'porter unicode61'
            );",
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
            Ok((
                r.get::<_, String>(0)?,
                (r.get::<_, i64>(1)?, r.get::<_, i64>(2)?),
            ))
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
        let lookups = scan::build_lookups(&self.roots);

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

        // Re-parse changed files in parallel, with the searchable body per
        // session (read with a cap so a giant transcript stays bounded).
        let parsed: Vec<(String, Vec<ScannedBody>)> = changed
            .par_iter()
            .map(|&c| {
                let rows = scan::parse_candidate(c, &lookups)
                    .into_iter()
                    .map(|(s, mt, sz)| {
                        let body = content_body(&s);
                        (s, mt, sz, body)
                    })
                    .collect::<Vec<_>>();
                (c.path.to_string_lossy().to_string(), rows)
            })
            .collect();

        // Files that vanished from disk. Disjoint from `changed` (a subset of
        // `present`), so the two delete passes never overlap.
        let to_delete: Vec<String> = existing
            .keys()
            .filter(|p| !present.contains(*p))
            .cloned()
            .collect();

        let mut parsed_rows = 0usize;
        let tx = self.conn.transaction()?;
        for (file_path, rows) in &parsed {
            // A changed file that parsed to nothing is treated as transient
            // (mid-write or briefly unreadable): keep its previous rows and
            // retry next refresh, rather than wiping good data. Replacing only
            // when we have rows also evicts stale per-directory shell rows.
            if rows.is_empty() {
                continue;
            }
            tx.execute(
                "DELETE FROM content_fts WHERE session_id IN \
                 (SELECT id FROM sessions WHERE file_path = ?1)",
                params![file_path],
            )?;
            tx.execute(
                "DELETE FROM sessions WHERE file_path = ?1",
                params![file_path],
            )?;
            for (s, mtime, size, body) in rows {
                upsert(&tx, s, *mtime, *size)?;
                tx.execute(
                    "INSERT INTO content_fts (session_id, body) VALUES (?1, ?2)",
                    params![s.id, body],
                )?;
                parsed_rows += 1;
            }
        }
        for path in &to_delete {
            tx.execute(
                "DELETE FROM content_fts WHERE session_id IN \
                 (SELECT id FROM sessions WHERE file_path = ?1)",
                params![path],
            )?;
            tx.execute("DELETE FROM sessions WHERE file_path = ?1", params![path])?;
        }
        tx.commit()?;

        Ok(RefreshStats {
            parsed: parsed_rows,
            deleted: to_delete.len(),
            total: candidates.len(),
        })
    }
}

/// The searchable body for a session: its first messages (capped), joined.
/// Reused for the content FTS index. Empty on a read error.
fn content_body(s: &Session) -> String {
    let page = match transcript::read(s, 0, FTS_MSG_CAP) {
        Ok(p) => p,
        Err(_) => return String::new(),
    };
    let mut buf = String::new();
    for m in &page.messages {
        if !buf.is_empty() {
            buf.push('\n');
        }
        buf.push_str(&m.text);
        if buf.len() >= FTS_BODY_MAX_CHARS {
            break;
        }
    }
    if buf.chars().count() > FTS_BODY_MAX_CHARS {
        buf = buf.chars().take(FTS_BODY_MAX_CHARS).collect();
    }
    buf
}

/// Unique row key. Shell logs split into one row per directory, so they include
/// the directory; Claude/Codex are one row per file.
fn session_key(s: &Session) -> String {
    match s.source {
        Source::Shell => format!("{}#{}", s.file_path, s.cwd),
        // Gemini logs and the opencode DB pack many sessions into one file.
        Source::Gemini | Source::Opencode => format!("{}#{}", s.file_path, s.id),
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
        source: Source::from_tag(&source_str).unwrap_or(Source::Shell),
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
