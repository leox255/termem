pub mod claude;
pub mod codex;
pub mod shell;

use crate::model::{Session, Source};
use std::collections::HashMap;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

/// A file on disk that may contain a session, with cheap stat info used to
/// decide whether it needs re-parsing.
pub struct Candidate {
    pub path: PathBuf,
    pub source: Source,
    pub mtime_ms: i64,
    pub size: i64,
}

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

pub fn mtime_ms(meta: &Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn collect_files(root: &Path, ext: &str, source: Source, out: &mut Vec<Candidate>) {
    if !root.exists() {
        return;
    }
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().is_some_and(|x| x == ext) {
            if let Ok(meta) = entry.metadata() {
                out.push(Candidate {
                    path: path.to_path_buf(),
                    source,
                    mtime_ms: mtime_ms(&meta),
                    size: meta.len() as i64,
                });
            }
        }
    }
}

/// The directories scanned for each source. `None` disables a source. Override
/// the defaults to point termem at non-standard or synced session locations
/// (and to test incremental behavior against static fixtures).
#[derive(Debug, Clone)]
pub struct ScanRoots {
    pub claude: Option<PathBuf>,
    pub codex: Option<PathBuf>,
    pub shell: Option<PathBuf>,
}

impl ScanRoots {
    /// Standard per-tool locations under `$HOME`.
    pub fn home() -> Self {
        let h = home();
        ScanRoots {
            claude: Some(h.join(".claude/projects")),
            codex: Some(h.join(".codex/sessions")),
            shell: Some(h.join(".termem/shell")),
        }
    }
}

impl Default for ScanRoots {
    fn default() -> Self {
        ScanRoots::home()
    }
}

/// All session files across every enabled source.
pub fn gather_candidates(roots: &ScanRoots) -> Vec<Candidate> {
    let mut out = Vec::new();
    if let Some(p) = &roots.claude {
        collect_files(p, "jsonl", Source::Claude, &mut out);
    }
    if let Some(p) = &roots.codex {
        collect_files(p, "jsonl", Source::Codex, &mut out);
    }
    if let Some(p) = &roots.shell {
        collect_files(p, "log", Source::Shell, &mut out);
    }
    out
}

/// A parsed session plus the file stat info (mtime, size) to cache.
pub type ScannedRow = (Session, i64, i64);

/// Parse one candidate into zero or more normalized [`Session`]s, each with the
/// stat info to cache. Claude/Codex yield at most one; a shell log yields one
/// per directory it touched.
pub fn parse_candidate(c: &Candidate, thread_map: &HashMap<String, String>) -> Vec<ScannedRow> {
    let sessions: Vec<Session> = match c.source {
        Source::Claude => claude::parse(&c.path, c.mtime_ms)
            .ok()
            .flatten()
            .into_iter()
            .collect(),
        Source::Codex => codex::parse(&c.path, c.mtime_ms, thread_map)
            .ok()
            .flatten()
            .into_iter()
            .collect(),
        Source::Shell => shell::parse(&c.path, c.mtime_ms).unwrap_or_default(),
    };
    sessions
        .into_iter()
        .map(|s| (s, c.mtime_ms, c.size))
        .collect()
}

/// Parse an RFC3339 timestamp (e.g. `2026-06-15T11:13:13.977Z`) to epoch ms.
pub fn parse_ms(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp_millis())
}
