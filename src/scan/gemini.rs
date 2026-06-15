//! Parser for Gemini CLI sessions.
//!
//! Layout: `~/.gemini/tmp/<projectKey>/chats/session-<ts>-<id>.jsonl`. One file
//! is one resumable session. Line 1 is metadata `{sessionId, startTime, ...}`.
//! Messages are stored either as top-level lines (`type` "user"|"gemini" +
//! `content[].text`) or inside `$set`/`$push` `messages` snapshots, sometimes
//! both, so they are collected from all of those and deduped by message id.
//! `~/.gemini/projects.json` maps each project directory to its `tmp/<key>`
//! name. Resume with `gemini --session-file <path>`.

use crate::model::{truncate_title, Session, Source};
use crate::scan::parse_ms;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
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
    // tmp/<projectKey>/chats/session-*.jsonl -> key is two parents up.
    let key = path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let cwd = match project_map.get(&key) {
        Some(c) => c.clone(),
        None => return Ok(Vec::new()), // unmapped key: not attributable to a dir
    };
    let file = File::open(path)?;
    Ok(parse_reader(
        BufReader::new(file),
        &cwd,
        mtime_ms,
        path.to_string_lossy().as_ref(),
    ))
}

/// A single message: role ("user" | "gemini") and its text.
pub struct GMsg {
    pub role: String,
    pub text: String,
    pub ts: Option<String>,
}

/// Extract the joined text of a message's `content` (array of parts or a string).
fn content_text(v: &Value) -> String {
    match v.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(arr)) => {
            let mut buf = String::new();
            for el in arr {
                if let Some(t) = el.get("text").and_then(|x| x.as_str()) {
                    buf.push_str(t);
                    buf.push('\n');
                }
            }
            buf
        }
        _ => String::new(),
    }
}

/// Build a message from a top-level line or a snapshot element, if it is one.
fn gmsg(v: &Value) -> Option<GMsg> {
    let role = match v.get("type").and_then(|x| x.as_str()) {
        Some("user") => "user",
        Some("gemini") => "gemini",
        _ => return None,
    };
    Some(GMsg {
        role: role.to_string(),
        text: content_text(v).trim().to_string(),
        ts: v
            .get("timestamp")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
    })
}

/// Read a chats file into (session_id, started_ms, ordered messages). Prefer the
/// top-level message log when present; otherwise fall back to the `$set`/`$push`
/// snapshots (some sessions store the conversation only there).
pub fn collect_session<R: BufRead>(reader: R) -> (String, i64, Vec<GMsg>) {
    let mut id = String::new();
    let mut started = 0i64;
    let mut top: Vec<GMsg> = Vec::new();
    let mut snapshot: Vec<GMsg> = Vec::new();

    for line in reader.lines().map_while(Result::ok) {
        if line.is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if id.is_empty() {
            if let Some(sid) = v.get("sessionId").and_then(|x| x.as_str()) {
                id = sid.to_string();
                started = v
                    .get("startTime")
                    .and_then(|x| x.as_str())
                    .and_then(parse_ms)
                    .unwrap_or(0);
                continue;
            }
        }
        if let Some(g) = gmsg(&v) {
            top.push(g);
        }
        // A `$set.messages` is a full replacement of the message list.
        if let Some(arr) = v.pointer("/$set/messages").and_then(|x| x.as_array()) {
            snapshot = arr.iter().filter_map(gmsg).collect();
        }
        // A `$push.messages` appends.
        if let Some(arr) = v.pointer("/$push/messages").and_then(|x| x.as_array()) {
            snapshot.extend(arr.iter().filter_map(gmsg));
        }
    }
    let msgs = if top.is_empty() { snapshot } else { top };
    (id, started, msgs)
}

/// One session per chats file. Empty if the file has no `sessionId` metadata.
pub fn parse_reader<R: BufRead>(
    reader: R,
    cwd: &str,
    mtime_ms: i64,
    file_path: &str,
) -> Vec<Session> {
    let (id, started, msgs) = collect_session(reader);
    if id.is_empty() {
        return Vec::new();
    }

    let mut first: Option<String> = None;
    let mut last_user: Option<String> = None;
    let mut count = 0i64;
    for m in &msgs {
        // The injected <session_context> preamble is not a real turn. Gemini
        // assistant turns persist an empty `content`, so count them regardless
        // of text; only use text to pick the title and last prompt.
        if m.role == "user" && m.text.starts_with("<session_context>") {
            continue;
        }
        count += 1;
        if m.role == "user" && !m.text.is_empty() {
            if first.is_none() {
                first = Some(m.text.clone());
            }
            last_user = Some(m.text.clone());
        }
    }
    if count == 0 {
        return Vec::new(); // empty / context-only session: not worth indexing
    }

    let started = if started > 0 { started } else { mtime_ms };
    let first_prompt = first.unwrap_or_default();
    let title = if first_prompt.is_empty() {
        "(gemini session)".to_string()
    } else {
        truncate_title(&first_prompt)
    };
    vec![Session {
        id,
        source: Source::Gemini,
        file_path: file_path.to_string(),
        cwd: cwd.to_string(),
        title,
        last_prompt: last_user.unwrap_or_else(|| first_prompt.clone()),
        first_prompt,
        model: None,
        git_branch: None,
        started_at: started,
        updated_at: mtime_ms,
        msg_count: count,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn parses_messages_from_top_level_lines() {
        let content = "{\"sessionId\":\"abc-123\",\"startTime\":\"2026-06-15T13:00:00.000Z\",\"kind\":\"main\"}\n\
{\"$set\":{\"messages\":[]}}\n\
{\"type\":\"user\",\"content\":[{\"text\":\"<session_context>\\nsetup\"}]}\n\
{\"type\":\"user\",\"content\":[{\"text\":\"what are we working on?\"}]}\n\
{\"type\":\"gemini\",\"content\":[{\"text\":\"your project\"}]}\n\
{\"type\":\"user\",\"content\":[{\"text\":\"ship it\"}]}\n";
        let s = &parse_reader(Cursor::new(content), "/work/proj", 999, "/g/chats/s.jsonl")[0];
        assert_eq!(s.id, "abc-123");
        assert_eq!(s.cwd, "/work/proj");
        assert_eq!(s.title, "what are we working on?"); // skips <session_context>
        assert_eq!(s.last_prompt, "ship it");
        assert_eq!(s.msg_count, 3); // context preamble excluded
        assert!(s.started_at > 0);
    }

    #[test]
    fn context_only_session_is_skipped() {
        // A session whose only message is the injected context is not indexed.
        let content = "{\"sessionId\":\"s3\",\"startTime\":\"2026-06-15T13:00:00.000Z\"}\n\
{\"$set\":{\"messages\":[{\"id\":\"m1\",\"type\":\"user\",\"content\":[{\"text\":\"<session_context>\\nsetup\"}]}]}}\n";
        assert!(parse_reader(Cursor::new(content), "/w", 0, "/f.jsonl").is_empty());
    }

    #[test]
    fn parses_messages_from_set_snapshot() {
        // The whole conversation lives in a $set.messages snapshot (no top-level
        // type lines), deduped by message id.
        let content = "{\"sessionId\":\"s2\",\"startTime\":\"2026-06-15T13:00:00.000Z\"}\n\
{\"$set\":{\"messages\":[\
{\"id\":\"m1\",\"type\":\"user\",\"content\":[{\"text\":\"fix the build\"}]},\
{\"id\":\"m2\",\"type\":\"gemini\",\"content\":[{\"text\":\"done\"}]}]}}\n\
{\"$set\":{\"messages\":[\
{\"id\":\"m1\",\"type\":\"user\",\"content\":[{\"text\":\"fix the build\"}]},\
{\"id\":\"m2\",\"type\":\"gemini\",\"content\":[{\"text\":\"done\"}]}]}}\n";
        let s = &parse_reader(Cursor::new(content), "/w", 0, "/f.jsonl")[0];
        assert_eq!(s.title, "fix the build");
        assert_eq!(s.msg_count, 2, "deduped by id across the two snapshots");
    }

    #[test]
    fn non_session_file_yields_nothing() {
        assert!(parse_reader(Cursor::new("{\"foo\":1}\n"), "/w", 0, "/f.jsonl").is_empty());
    }
}
