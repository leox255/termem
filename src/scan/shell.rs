//! Parser for termem's own shell session logs.
//!
//! Written by the `termem init` hook, one tab-separated record per command:
//! `<epoch_seconds>\t<cwd>\t<command>`. A single shell session usually visits
//! several directories, so it is indexed once per directory it touched: each
//! per-directory entry shows up when you are in that directory.

use crate::model::{truncate_title, Session, Source};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub fn parse(path: &Path, mtime_ms: i64) -> anyhow::Result<Vec<Session>> {
    let id = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let file = File::open(path)?;
    Ok(parse_reader(
        BufReader::new(file),
        id,
        path.to_string_lossy().to_string(),
        mtime_ms,
    ))
}

struct DirAcc {
    first_cmd: String,
    last_cmd: String,
    started: i64,
    updated: i64,
    count: i64,
}

/// Returns one [`Session`] per distinct directory seen in the log.
pub fn parse_reader<R: BufRead>(
    reader: R,
    id: String,
    file_path: String,
    mtime_ms: i64,
) -> Vec<Session> {
    let mut dirs: HashMap<String, DirAcc> = HashMap::new();

    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, '\t');
        let ts = parts.next().unwrap_or("");
        let dir = parts.next().unwrap_or("");
        let cmd = parts.next().unwrap_or("").trim();
        if dir.is_empty() || cmd.is_empty() {
            continue;
        }
        let acc = dirs.entry(dir.to_string()).or_insert(DirAcc {
            first_cmd: String::new(),
            last_cmd: String::new(),
            started: i64::MAX,
            updated: 0,
            count: 0,
        });
        if acc.first_cmd.is_empty() {
            acc.first_cmd = cmd.to_string();
        }
        acc.last_cmd = cmd.to_string();
        acc.count += 1;
        if let Ok(secs) = ts.parse::<i64>() {
            // checked_mul: a corrupt/huge value in the log must not overflow
            // (panic in debug, wrap to a negative timestamp in release).
            if let Some(ms) = secs.checked_mul(1000) {
                acc.started = acc.started.min(ms);
                acc.updated = acc.updated.max(ms);
            }
        }
    }

    dirs.into_iter()
        .map(|(dir, a)| {
            let started = if a.started == i64::MAX {
                mtime_ms
            } else {
                a.started
            };
            let updated = if a.updated == 0 { mtime_ms } else { a.updated };
            Session {
                id: id.clone(),
                source: Source::Shell,
                file_path: file_path.clone(),
                cwd: dir,
                title: truncate_title(&a.first_cmd),
                first_prompt: a.first_cmd,
                last_prompt: a.last_cmd,
                model: None,
                git_branch: None,
                started_at: started,
                updated_at: updated,
                msg_count: a.count,
                bypass: false,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn one_session_per_directory() {
        let lines = "1718450000\t/work/proj\tgit status\n\
                     1718450050\t/work/proj/sub\tcargo test\n\
                     1718450090\t/work/proj\tgit commit\n";
        let sessions = parse_reader(Cursor::new(lines), "sess1".into(), "/f.log".into(), 0);
        assert_eq!(sessions.len(), 2, "one entry per distinct directory");

        let proj = sessions.iter().find(|s| s.cwd == "/work/proj").unwrap();
        assert_eq!(proj.title, "git status");
        assert_eq!(proj.last_prompt, "git commit");
        assert_eq!(proj.msg_count, 2);
        assert_eq!(proj.started_at, 1_718_450_000_000);

        let sub = sessions.iter().find(|s| s.cwd == "/work/proj/sub").unwrap();
        assert_eq!(sub.title, "cargo test");
        assert_eq!(sub.msg_count, 1);
    }
}
