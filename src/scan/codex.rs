//! Parser for Codex CLI rollout transcripts.
//!
//! Layout: `~/.codex/sessions/YYYY/MM/DD/rollout-<iso>-<uuid>.jsonl`.
//! Each line is `{ "type", "timestamp", "payload" }`. Line `type`s:
//!   - `session_meta`: `payload.{id, cwd, timestamp, cli_version, model_provider}`.
//!   - `turn_context`: `payload.{model, cwd, ...}`.
//!   - `event_msg`: `payload.type` ∈ {user_message, agent_message, ...}.
//!   - `response_item`: `payload.{type: "message", role, content[]}`.
//!
//! Titles prefer `~/.codex/session_index.jsonl` `thread_name`, then the first
//! real user prompt.

use crate::model::{truncate_title, Session, Source};
use crate::scan::parse_ms;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Load the (partial) `id -> thread_name` map Codex maintains alongside its
/// sessions directory (`<codex_root>/../session_index.jsonl`). Empty if the
/// codex source is disabled or the index file is absent.
pub fn load_thread_map(codex_root: Option<&Path>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let path = match codex_root.and_then(|r| r.parent()) {
        Some(dir) => dir.join("session_index.jsonl"),
        None => return map,
    };
    let file = match File::open(&path) {
        Ok(f) => f,
        Err(_) => return map,
    };
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(&line) {
            if let (Some(id), Some(name)) = (
                v.get("id").and_then(|x| x.as_str()),
                v.get("thread_name").and_then(|x| x.as_str()),
            ) {
                if !name.trim().is_empty() {
                    map.insert(id.to_string(), name.trim().to_string());
                }
            }
        }
    }
    map
}

pub fn parse(
    path: &Path,
    mtime_ms: i64,
    thread_map: &HashMap<String, String>,
) -> anyhow::Result<Option<Session>> {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    Ok(parse_reader(
        reader,
        stem,
        path.to_string_lossy().to_string(),
        mtime_ms,
        thread_map,
    ))
}

/// Recover the uuid from a `rollout-<iso>-<uuid>` stem (the uuid is the last
/// five dash-delimited groups).
fn uuid_from_stem(stem: &str) -> String {
    let parts: Vec<&str> = stem.split('-').collect();
    if parts.len() >= 5 {
        parts[parts.len() - 5..].join("-")
    } else {
        stem.to_string()
    }
}

pub fn parse_reader<R: BufRead>(
    reader: R,
    stem: String,
    file_path: String,
    mtime_ms: i64,
    thread_map: &HashMap<String, String>,
) -> Option<Session> {
    let mut id = uuid_from_stem(&stem);
    let mut cwd: Option<String> = None;
    let mut model: Option<String> = None;
    let mut first_prompt: Option<String> = None;
    let mut first_prompt_fallback: Option<String> = None;
    let mut last_user: Option<String> = None;
    let mut started = i64::MAX;
    let mut updated = 0i64;
    let mut msg_count = 0i64;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(ts) = v.get("timestamp").and_then(|x| x.as_str()) {
            if let Some(ms) = parse_ms(ts) {
                started = started.min(ms);
                updated = updated.max(ms);
            }
        }

        let payload = v.get("payload");
        match v.get("type").and_then(|x| x.as_str()).unwrap_or("") {
            "session_meta" => {
                if let Some(p) = payload {
                    if let Some(i) = p.get("id").and_then(|x| x.as_str()) {
                        if !i.is_empty() {
                            id = i.to_string();
                        }
                    }
                    if cwd.is_none() {
                        if let Some(c) = p.get("cwd").and_then(|x| x.as_str()) {
                            cwd = Some(c.to_string());
                        }
                    }
                }
            }
            "turn_context" => {
                if let Some(p) = payload {
                    if model.is_none() {
                        if let Some(m) = p.get("model").and_then(|x| x.as_str()) {
                            if !m.is_empty() {
                                model = Some(m.to_string());
                            }
                        }
                    }
                    if cwd.is_none() {
                        if let Some(c) = p.get("cwd").and_then(|x| x.as_str()) {
                            cwd = Some(c.to_string());
                        }
                    }
                }
            }
            "event_msg" => {
                if let Some(p) = payload {
                    match p.get("type").and_then(|x| x.as_str()).unwrap_or("") {
                        "user_message" => {
                            if let Some(m) = p.get("message").and_then(|x| x.as_str()) {
                                let m = m.trim();
                                if !m.is_empty() && !is_injected(m) {
                                    msg_count += 1;
                                    if first_prompt.is_none() {
                                        first_prompt = Some(m.to_string());
                                    }
                                    last_user = Some(m.to_string());
                                }
                            }
                        }
                        "agent_message" => msg_count += 1,
                        _ => {}
                    }
                }
            }
            "response_item" => {
                // Fallback prompt source when no event_msg user_message exists.
                if let Some(p) = payload {
                    let is_user_msg = p.get("type").and_then(|x| x.as_str()) == Some("message")
                        && p.get("role").and_then(|x| x.as_str()) == Some("user");
                    if is_user_msg && first_prompt_fallback.is_none() {
                        if let Some(text) = extract_content_text(p.get("content")) {
                            if !is_injected(&text) {
                                first_prompt_fallback = Some(text);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let cwd = cwd?;
    if started == i64::MAX {
        started = if updated > 0 { updated } else { mtime_ms };
    }
    if updated == 0 {
        updated = mtime_ms;
    }
    let first_prompt = first_prompt.or(first_prompt_fallback).unwrap_or_default();
    let last_prompt = last_user.unwrap_or_else(|| first_prompt.clone());
    let title = thread_map
        .get(&id)
        .cloned()
        .or_else(|| {
            if first_prompt.is_empty() {
                None
            } else {
                Some(truncate_title(&first_prompt))
            }
        })
        .unwrap_or_else(|| "(codex session)".to_string());

    Some(Session {
        id,
        source: Source::Codex,
        file_path,
        cwd,
        title,
        first_prompt,
        last_prompt,
        model,
        git_branch: None,
        started_at: started,
        updated_at: updated,
        msg_count,
    })
}

fn extract_content_text(content: Option<&Value>) -> Option<String> {
    let arr = content?.as_array()?;
    let mut buf = String::new();
    for el in arr {
        let t = el
            .get("text")
            .or_else(|| el.get("input_text"))
            .and_then(|x| x.as_str());
        if let Some(t) = t {
            buf.push_str(t);
            buf.push('\n');
        }
    }
    let buf = buf.trim().to_string();
    if buf.is_empty() {
        None
    } else {
        Some(buf)
    }
}

/// Codex injects environment/instruction blocks as "user" turns; skip them so
/// the title reflects the human's actual first ask.
fn is_injected(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with('<')
        || t.starts_with("IMPORTANT: Do NOT read")
        || t.starts_with("You are ")
        || t.starts_with("# ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn empty_map() -> HashMap<String, String> {
        HashMap::new()
    }

    #[test]
    fn parses_session_meta_and_first_real_prompt() {
        let lines = r#"{"type":"session_meta","timestamp":"2026-06-01T13:20:08.918Z","payload":{"id":"019e8357-3f8f-77e0-95f5-64b20ece1e79","cwd":"/Users/x/proj","cli_version":"0.130.0"}}
{"type":"turn_context","timestamp":"2026-06-01T13:20:09.000Z","payload":{"model":"gpt-5","cwd":"/Users/x/proj"}}
{"type":"event_msg","timestamp":"2026-06-01T13:20:10.000Z","payload":{"type":"user_message","message":"<environment_context>stuff</environment_context>"}}
{"type":"event_msg","timestamp":"2026-06-01T13:20:11.000Z","payload":{"type":"user_message","message":"refactor the importer"}}
{"type":"event_msg","timestamp":"2026-06-01T13:20:20.000Z","payload":{"type":"agent_message","message":"done"}}
"#;
        let s = parse_reader(
            Cursor::new(lines),
            "rollout-2026-06-01T16-19-53-019e8357-3f8f-77e0-95f5-64b20ece1e79".into(),
            "/f.jsonl".into(),
            0,
            &empty_map(),
        )
        .unwrap();
        assert_eq!(s.id, "019e8357-3f8f-77e0-95f5-64b20ece1e79");
        assert_eq!(s.cwd, "/Users/x/proj");
        assert_eq!(s.model.as_deref(), Some("gpt-5"));
        assert_eq!(s.first_prompt, "refactor the importer");
        assert_eq!(s.title, "refactor the importer");
        assert_eq!(s.msg_count, 2);
    }

    #[test]
    fn thread_name_wins_for_title() {
        let mut map = empty_map();
        map.insert("019e8357-3f8f-77e0-95f5-64b20ece1e79".into(), "Nice Title".into());
        let lines = r#"{"type":"session_meta","timestamp":"2026-06-01T13:20:08.918Z","payload":{"id":"019e8357-3f8f-77e0-95f5-64b20ece1e79","cwd":"/p"}}
{"type":"event_msg","timestamp":"2026-06-01T13:20:11.000Z","payload":{"type":"user_message","message":"hello there"}}
"#;
        let s = parse_reader(Cursor::new(lines), "rollout-x".into(), "/f.jsonl".into(), 0, &map)
            .unwrap();
        assert_eq!(s.title, "Nice Title");
    }

    #[test]
    fn uuid_extraction() {
        assert_eq!(
            uuid_from_stem("rollout-2026-06-01T16-19-53-019e8357-3f8f-77e0-95f5-64b20ece1e79"),
            "019e8357-3f8f-77e0-95f5-64b20ece1e79"
        );
    }
}
