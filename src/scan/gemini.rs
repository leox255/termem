//! Parser for Gemini CLI prompt logs.
//!
//! Layout: `~/.gemini/tmp/<projectKey>/logs.json`, a JSON array of
//! `{sessionId, messageId, type, message, timestamp}` records (user turns only).
//! `~/.gemini/projects.json` maps each project directory to its key, which is
//! the `tmp/<key>` directory name. One log holds several sessions (by
//! sessionId). Gemini has no resume-by-id, so these are browse + reopen.

use crate::model::{truncate_title, Session, Source};
use crate::scan::parse_ms;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Load `key -> directory` by inverting `~/.gemini/projects.json` (stored as
/// `directory -> key`). `gemini_tmp` is `.../.gemini/tmp`.
pub fn load_project_map(gemini_tmp: Option<&Path>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let path = match gemini_tmp.and_then(|t| t.parent()) {
        Some(dir) => dir.join("projects.json"),
        None => return map,
    };
    let mut buf = String::new();
    if File::open(&path)
        .and_then(|mut f| f.read_to_string(&mut buf))
        .is_err()
    {
        return map;
    }
    if let Ok(v) = serde_json::from_str::<Value>(&buf) {
        if let Some(obj) = v.get("projects").and_then(|p| p.as_object()) {
            for (dir, key) in obj {
                if let Some(k) = key.as_str() {
                    map.insert(k.to_string(), dir.clone());
                }
            }
        }
    }
    map
}

pub fn parse(
    path: &Path,
    mtime_ms: i64,
    project_map: &HashMap<String, String>,
) -> anyhow::Result<Vec<Session>> {
    // The project key is the parent directory name: tmp/<key>/logs.json.
    let key = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let cwd = match project_map.get(&key) {
        Some(c) => c.clone(),
        None => return Ok(Vec::new()), // unmapped key: not attributable to a dir
    };
    let mut buf = String::new();
    File::open(path)?.read_to_string(&mut buf)?;
    Ok(parse_str(
        &buf,
        &cwd,
        mtime_ms,
        path.to_string_lossy().as_ref(),
    ))
}

struct Acc {
    first: String,
    last: String,
    started: i64,
    updated: i64,
    count: i64,
}

/// Split a logs.json array into one [`Session`] per `sessionId`.
pub fn parse_str(content: &str, cwd: &str, mtime_ms: i64, file_path: &str) -> Vec<Session> {
    let arr: Vec<Value> = match serde_json::from_str(content) {
        Ok(Value::Array(a)) => a,
        _ => return Vec::new(),
    };
    let mut sessions: HashMap<String, Acc> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for rec in &arr {
        let sid = match rec.get("sessionId").and_then(|x| x.as_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let msg = rec
            .get("message")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim();
        let ts = rec
            .get("timestamp")
            .and_then(|x| x.as_str())
            .and_then(parse_ms);
        let acc = sessions.entry(sid.clone()).or_insert_with(|| {
            order.push(sid.clone());
            Acc {
                first: String::new(),
                last: String::new(),
                started: i64::MAX,
                updated: 0,
                count: 0,
            }
        });
        if !msg.is_empty() {
            if acc.first.is_empty() {
                acc.first = msg.to_string();
            }
            acc.last = msg.to_string();
            acc.count += 1;
        }
        if let Some(ms) = ts {
            acc.started = acc.started.min(ms);
            acc.updated = acc.updated.max(ms);
        }
    }

    order
        .into_iter()
        .filter_map(|sid| {
            let a = sessions.remove(&sid)?;
            if a.count == 0 {
                return None;
            }
            let started = if a.started == i64::MAX {
                mtime_ms
            } else {
                a.started
            };
            let updated = if a.updated == 0 { mtime_ms } else { a.updated };
            Some(Session {
                id: sid,
                source: Source::Gemini,
                file_path: file_path.to_string(),
                cwd: cwd.to_string(),
                title: truncate_title(&a.first),
                first_prompt: a.first,
                last_prompt: a.last,
                model: None,
                git_branch: None,
                started_at: started,
                updated_at: updated,
                msg_count: a.count,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_logs_by_session() {
        let content = r#"[
          {"sessionId":"a","messageId":0,"type":"user","message":"first ask","timestamp":"2026-05-14T13:00:00.000Z"},
          {"sessionId":"a","messageId":1,"type":"user","message":"second ask","timestamp":"2026-05-14T13:05:00.000Z"},
          {"sessionId":"b","messageId":0,"type":"user","message":"other session","timestamp":"2026-05-14T14:00:00.000Z"}
        ]"#;
        let sessions = parse_str(content, "/work/proj", 0, "/g/logs.json");
        assert_eq!(sessions.len(), 2);
        let a = sessions.iter().find(|s| s.id == "a").unwrap();
        assert_eq!(a.cwd, "/work/proj");
        assert_eq!(a.title, "first ask");
        assert_eq!(a.last_prompt, "second ask");
        assert_eq!(a.msg_count, 2);
        assert_eq!(a.source, Source::Gemini);
        let b = sessions.iter().find(|s| s.id == "b").unwrap();
        assert_eq!(b.msg_count, 1);
    }

    #[test]
    fn maps_project_key_to_directory() {
        let mut map = HashMap::new();
        map.insert("loopsy".to_string(), "/Users/x/loopsy".to_string());
        // inverse of {"/Users/x/loopsy":"loopsy"}
        assert_eq!(map.get("loopsy").unwrap(), "/Users/x/loopsy");
    }
}
