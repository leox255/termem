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

const SCHEMA_VERSION: i64 = 5;

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

pub struct RelinkStats {
    /// Cached session rows repointed onto the new path.
    pub sessions: usize,
    /// Board posts repointed onto the new path.
    pub board: usize,
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

        // Per-directory message board: agents post short notes scoped to a
        // working directory; other sessions in that tree read them. Durable
        // agent-authored data, so (like summaries) it is never dropped when the
        // session cache is rebuilt for a new SCHEMA_VERSION.
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS board (
                id          INTEGER PRIMARY KEY,
                cwd         TEXT NOT NULL,
                author      TEXT,
                kind        TEXT NOT NULL DEFAULT 'note',
                body        TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                resolved_at INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_board_cwd ON board(cwd);
            CREATE INDEX IF NOT EXISTS idx_board_created ON board(created_at);",
        )?;
        // Soft-resolve column (0.6.1). Best-effort ALTER so a 0.6.0 board (which
        // lacks it) upgrades in place without dropping its posts.
        let _ = self
            .conn
            .execute("ALTER TABLE board ADD COLUMN resolved_at INTEGER", []);

        // Folder-move remap rules (0.6.3). When a project directory is moved or
        // renamed, the path baked into each tool's session file is now stale.
        // `termem relink <old> <new>` records a rule here; it is applied on every
        // re-parse (see `refresh`) and is durable — like summaries/board, it
        // survives a session-cache rebuild for a new SCHEMA_VERSION.
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS path_remap (
                old_path   TEXT PRIMARY KEY,
                new_path   TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );",
        )?;

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
                bypass      INTEGER NOT NULL DEFAULT 0,
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
        // Active folder-move rules, applied to each session's cwd as it is written.
        let remap = self.load_remap()?;

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
                // Repoint a re-parsed session onto its current path if a relink
                // rule covers the (now-stale) cwd recorded in the source file.
                let remapped;
                let s = match remap_cwd(&s.cwd, &remap) {
                    Some(cwd) => {
                        remapped = Session { cwd, ..s.clone() };
                        &remapped
                    }
                    None => s,
                };
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

    /// Record a folder move and repoint everything under `old` to `new`.
    ///
    /// Two parts, both needed: the rule is persisted in `path_remap` so future
    /// re-parses stay remapped (the source files still hold the old path), and
    /// the rows already cached are rewritten now — the incremental refresh skips
    /// unchanged source files, so a move would otherwise never be picked up.
    /// Matching is exact-or-subtree, so a session that ran in a child directory
    /// of the moved folder moves with it.
    pub fn relink(&mut self, old: &str, new: &str) -> Result<RelinkStats> {
        let now = chrono::Utc::now().timestamp_millis();
        let sep = std::path::MAIN_SEPARATOR.to_string();
        // `<old><sep>%` matches descendants; the bare `cwd = old` arm catches the
        // folder itself. substr() is 1-based and char-counted, so cut past `old`
        // yields the trailing `<sep><rest>` (empty for the exact match → just `new`).
        let like = format!(
            "{}{}%",
            crate::query::escape_like(old),
            crate::query::escape_like(&sep)
        );
        let cut = old.chars().count() as i64 + 1;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO path_remap (old_path, new_path, created_at) \
             VALUES (?1, ?2, ?3)",
            params![old, new, now],
        )?;
        // The shell `key` (file_path#cwd) is left stale here; it is cosmetic
        // (queries filter on cwd) and is rewritten whole on the next refresh of
        // that log, since refresh deletes a file's rows before re-inserting.
        let sessions = tx.execute(
            "UPDATE sessions SET cwd = ?2 || substr(cwd, ?3) \
             WHERE cwd = ?1 OR cwd LIKE ?4 ESCAPE '\\'",
            params![old, new, cut, like],
        )?;
        let board = tx.execute(
            "UPDATE board SET cwd = ?2 || substr(cwd, ?3) \
             WHERE cwd = ?1 OR cwd LIKE ?4 ESCAPE '\\'",
            params![old, new, cut, like],
        )?;
        tx.commit()?;
        Ok(RelinkStats { sessions, board })
    }

    /// All active relink rules as `(old, new, created_at_ms)`, newest first.
    pub fn remaps(&self) -> Result<Vec<(String, String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT old_path, new_path, created_at FROM path_remap ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Load the relink rules, longest `old` first so the most specific rule wins
    /// when several could match a path.
    fn load_remap(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT old_path, new_path FROM path_remap")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        out.sort_by_key(|(old, _)| std::cmp::Reverse(old.chars().count()));
        Ok(out)
    }
}

/// Apply the first matching relink rule to `cwd` (exact or subtree), or `None`
/// if no rule covers it. Rules are assumed pre-sorted most-specific-first.
fn remap_cwd(cwd: &str, rules: &[(String, String)]) -> Option<String> {
    let sep = std::path::MAIN_SEPARATOR;
    for (old, new) in rules {
        if cwd == old {
            return Some(new.clone());
        }
        let prefix = format!("{old}{sep}");
        if let Some(rest) = cwd.strip_prefix(&prefix) {
            return Some(format!("{new}{sep}{rest}"));
        }
    }
    None
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
             model, git_branch, started_at, updated_at, msg_count, bypass, file_mtime, file_size)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
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
            s.bypass,
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
        bypass: r.get("bypass")?,
    })
}

#[cfg(test)]
mod tests {
    use super::{remap_cwd, upsert, Index};
    use crate::model::{Session, Source};
    use crate::scan::ScanRoots;
    use rusqlite::params;

    fn empty_roots() -> ScanRoots {
        ScanRoots {
            claude: None,
            codex: None,
            gemini: None,
            opencode: None,
            shell: None,
        }
    }

    fn session(id: &str, file: &str, cwd: &str) -> Session {
        Session {
            id: id.to_string(),
            source: Source::Claude,
            file_path: file.to_string(),
            cwd: cwd.to_string(),
            title: "t".to_string(),
            first_prompt: "f".to_string(),
            last_prompt: "l".to_string(),
            model: None,
            git_branch: None,
            started_at: 1,
            updated_at: 1,
            msg_count: 1,
            bypass: false,
        }
    }

    fn cwd_of(idx: &Index, id: &str) -> String {
        idx.conn()
            .query_row("SELECT cwd FROM sessions WHERE id = ?1", params![id], |r| {
                r.get::<_, String>(0)
            })
            .unwrap()
    }

    #[test]
    fn remap_cwd_matches_exact_and_subtree() {
        let rules = vec![("/old/proj".to_string(), "/new/loc".to_string())];
        assert_eq!(remap_cwd("/old/proj", &rules).as_deref(), Some("/new/loc"));
        assert_eq!(
            remap_cwd("/old/proj/sub", &rules).as_deref(),
            Some("/new/loc/sub")
        );
        // A sibling that merely shares the prefix string must NOT match.
        assert_eq!(remap_cwd("/old/projector", &rules), None);
        assert_eq!(remap_cwd("/unrelated", &rules), None);
    }

    #[test]
    fn relink_repoints_sessions_and_board() {
        let tmp = std::env::temp_dir().join(format!("termem-relink-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        let mut idx = Index::open_with_roots(&tmp, empty_roots()).unwrap();

        upsert(idx.conn(), &session("a", "/f1", "/old/proj"), 0, 0).unwrap();
        upsert(idx.conn(), &session("b", "/f2", "/old/proj/sub"), 0, 0).unwrap();
        upsert(idx.conn(), &session("c", "/f3", "/other"), 0, 0).unwrap();
        idx.conn()
            .execute(
                "INSERT INTO board (cwd, kind, body, created_at) VALUES (?1, 'note', 'hi', 1)",
                params!["/old/proj/sub"],
            )
            .unwrap();

        let stats = idx.relink("/old/proj", "/new/loc").unwrap();
        assert_eq!(stats.sessions, 2);
        assert_eq!(stats.board, 1);

        assert_eq!(cwd_of(&idx, "a"), "/new/loc");
        assert_eq!(cwd_of(&idx, "b"), "/new/loc/sub");
        assert_eq!(cwd_of(&idx, "c"), "/other"); // untouched

        let board_cwd: String = idx
            .conn()
            .query_row("SELECT cwd FROM board LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(board_cwd, "/new/loc/sub");

        // The rule persists, so a freshly parsed session at the old path is
        // repointed on upsert via the refresh pipeline.
        let rules = idx.load_remap().unwrap();
        assert_eq!(
            remap_cwd("/old/proj/new-session", &rules).as_deref(),
            Some("/new/loc/new-session")
        );

        let _ = std::fs::remove_file(&tmp);
    }
}
