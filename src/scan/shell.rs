//! Parser for termem's own shell session logs.
//!
//! Written by the `termem init` hook, one tab-separated record per command:
//! `<epoch_seconds>\t<cwd>\t<command>`. A session file is one shell session;
//! its `cwd` is where it started (the directory of its first command).

use crate::model::{truncate_title, Session, Source};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub fn parse(path: &Path, mtime_ms: i64) -> anyhow::Result<Option<Session>> {
    let id = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    Ok(parse_reader(
        reader,
        id,
        path.to_string_lossy().to_string(),
        mtime_ms,
    ))
}

pub fn parse_reader<R: BufRead>(
    reader: R,
    id: String,
    file_path: String,
    mtime_ms: i64,
) -> Option<Session> {
    let mut cwd: Option<String> = None;
    let mut first_cmd: Option<String> = None;
    let mut last_cmd: Option<String> = None;
    let mut started = i64::MAX;
    let mut updated = 0i64;
    let mut count = 0i64;

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
        count += 1;
        if let Ok(secs) = ts.parse::<i64>() {
            let ms = secs * 1000;
            started = started.min(ms);
            updated = updated.max(ms);
        }
        if cwd.is_none() {
            cwd = Some(dir.to_string());
        }
        if first_cmd.is_none() {
            first_cmd = Some(cmd.to_string());
        }
        last_cmd = Some(cmd.to_string());
    }

    let cwd = cwd?;
    if started == i64::MAX {
        started = if updated > 0 { updated } else { mtime_ms };
    }
    if updated == 0 {
        updated = mtime_ms;
    }
    let first_prompt = first_cmd.unwrap_or_default();
    let title = if first_prompt.is_empty() {
        "(shell session)".to_string()
    } else {
        truncate_title(&first_prompt)
    };

    Some(Session {
        id,
        source: Source::Shell,
        file_path,
        cwd,
        title,
        first_prompt,
        last_prompt: last_cmd.unwrap_or_default(),
        model: None,
        git_branch: None,
        started_at: started,
        updated_at: updated,
        msg_count: count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn parses_tsv_log() {
        let lines = "1718450000\t/work/proj\tgit status\n1718450050\t/work/proj/sub\tcargo test\n";
        let s = parse_reader(Cursor::new(lines), "sess1".into(), "/f.log".into(), 0).unwrap();
        assert_eq!(s.cwd, "/work/proj");
        assert_eq!(s.title, "git status");
        assert_eq!(s.last_prompt, "cargo test");
        assert_eq!(s.msg_count, 2);
        assert_eq!(s.started_at, 1718450000_000);
    }
}
