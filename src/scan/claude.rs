//! Parser for Claude Code session transcripts.
//!
//! Layout: `~/.claude/projects/<encoded-cwd>/<session-uuid>.jsonl`.
//! One JSON object per line. Relevant line `type`s:
//!   - `user` / `assistant`: carry `cwd`, `gitBranch`, `sessionId`, `timestamp`,
//!     and `message.content` (string or content-block array).
//!   - `ai-title`: `{ "aiTitle": "..." }` — title Claude already generated.
//!   - `last-prompt`: `{ "lastPrompt": "..." }` — most recent user prompt.

use crate::model::{truncate_title, Session, Source};
use crate::scan::parse_ms;
use serde_json::Value;
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
    let mut git_branch: Option<String> = None;
    let mut title: Option<String> = None;
    let mut first_prompt: Option<String> = None;
    let mut last_prompt_field: Option<String> = None;
    let mut last_user: Option<String> = None;
    let mut model: Option<String> = None;
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
        if cwd.is_none() {
            if let Some(c) = v.get("cwd").and_then(|x| x.as_str()) {
                if !c.is_empty() {
                    cwd = Some(c.to_string());
                }
            }
        }
        if git_branch.is_none() {
            if let Some(g) = v.get("gitBranch").and_then(|x| x.as_str()) {
                if !g.is_empty() {
                    git_branch = Some(g.to_string());
                }
            }
        }

        match v.get("type").and_then(|x| x.as_str()).unwrap_or("") {
            "ai-title" => {
                if let Some(a) = v.get("aiTitle").and_then(|x| x.as_str()) {
                    if !a.trim().is_empty() {
                        title = Some(a.trim().to_string());
                    }
                }
            }
            "last-prompt" => {
                if let Some(p) = v.get("lastPrompt").and_then(|x| x.as_str()) {
                    if !p.trim().is_empty() {
                        last_prompt_field = Some(p.trim().to_string());
                    }
                }
            }
            "user" => {
                msg_count += 1;
                if let Some(text) = extract_user_text(&v) {
                    if first_prompt.is_none() {
                        first_prompt = Some(text.clone());
                    }
                    last_user = Some(text);
                }
            }
            "assistant" => {
                msg_count += 1;
                if model.is_none() {
                    if let Some(m) = v.pointer("/message/model").and_then(|x| x.as_str()) {
                        model = Some(m.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    // A file with no cwd isn't an attributable session (e.g. a stub).
    let cwd = cwd?;
    if started == i64::MAX {
        started = if updated > 0 { updated } else { mtime_ms };
    }
    if updated == 0 {
        updated = mtime_ms;
    }
    let first_prompt = first_prompt.unwrap_or_default();
    let title = title
        .or_else(|| {
            if first_prompt.is_empty() {
                None
            } else {
                Some(truncate_title(&first_prompt))
            }
        })
        .unwrap_or_else(|| "(untitled)".to_string());
    let last_prompt = last_prompt_field.or(last_user).unwrap_or_default();

    Some(Session {
        id,
        source: Source::Claude,
        file_path,
        cwd,
        title,
        first_prompt,
        last_prompt,
        model,
        git_branch,
        started_at: started,
        updated_at: updated,
        msg_count,
    })
}

/// Pull human-authored text out of a `user` line, skipping tool results,
/// meta lines, and injected command/system wrappers.
fn extract_user_text(v: &Value) -> Option<String> {
    if v.get("isMeta").and_then(|x| x.as_bool()) == Some(true) {
        return None;
    }
    let content = v.pointer("/message/content")?;
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(arr) => {
            let mut buf = String::new();
            for el in arr {
                if el.get("type").and_then(|x| x.as_str()) == Some("text") {
                    if let Some(t) = el.get("text").and_then(|x| x.as_str()) {
                        buf.push_str(t);
                        buf.push('\n');
                    }
                }
            }
            buf
        }
        _ => return None,
    };
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if text.starts_with("<command-")
        || text.starts_with("<local-command")
        || text.starts_with("<system-reminder")
        || text.starts_with("Caveat:")
    {
        return None;
    }
    Some(text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn extracts_ai_title_and_cwd() {
        let lines = r#"{"type":"mode","sessionId":"abc"}
{"type":"user","cwd":"/work/proj","gitBranch":"main","sessionId":"abc","timestamp":"2026-06-15T11:13:13.977Z","message":{"role":"user","content":"build the thing"}}
{"type":"assistant","timestamp":"2026-06-15T11:14:00.000Z","message":{"role":"assistant","model":"claude-opus-4-8","content":[{"type":"text","text":"ok"}]}}
{"type":"ai-title","aiTitle":"Build the thing","sessionId":"abc"}
{"type":"last-prompt","lastPrompt":"and ship it"}
"#;
        let s = parse_reader(Cursor::new(lines), "abc".into(), "/f.jsonl".into(), 0).unwrap();
        assert_eq!(s.title, "Build the thing");
        assert_eq!(s.cwd, "/work/proj");
        assert_eq!(s.git_branch.as_deref(), Some("main"));
        assert_eq!(s.first_prompt, "build the thing");
        assert_eq!(s.last_prompt, "and ship it");
        assert_eq!(s.model.as_deref(), Some("claude-opus-4-8"));
        assert_eq!(s.msg_count, 2);
        assert!(s.started_at > 0);
    }

    #[test]
    fn falls_back_to_first_prompt_when_no_title() {
        let lines = r#"{"type":"user","cwd":"/w","timestamp":"2026-06-15T11:13:13.977Z","message":{"content":"do a thing please"}}
"#;
        let s = parse_reader(Cursor::new(lines), "x".into(), "/f.jsonl".into(), 0).unwrap();
        assert_eq!(s.title, "do a thing please");
    }

    #[test]
    fn skips_meta_and_tool_result_users() {
        let lines = r#"{"type":"user","cwd":"/w","timestamp":"2026-06-15T11:13:13.977Z","message":{"content":"<command-name>/foo</command-name>"}}
{"type":"user","timestamp":"2026-06-15T11:13:14.000Z","message":{"content":[{"type":"tool_result","content":"x"}]}}
{"type":"user","timestamp":"2026-06-15T11:13:15.000Z","message":{"content":"the real prompt"}}
"#;
        let s = parse_reader(Cursor::new(lines), "x".into(), "/f.jsonl".into(), 0).unwrap();
        assert_eq!(s.first_prompt, "the real prompt");
    }

    #[test]
    fn no_cwd_means_not_a_session() {
        let lines = "{\"type\":\"mode\",\"sessionId\":\"abc\"}\n";
        assert!(parse_reader(Cursor::new(lines), "x".into(), "/f.jsonl".into(), 0).is_none());
    }
}
