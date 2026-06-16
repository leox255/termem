//! Read the real messages of a session for `get_session`, so an agent can
//! distil a summary or answer a detailed question. Read-only, per source.
//!
//! Reads stream and stop once enough messages for the requested page are
//! collected, so a single call never pulls a whole multi-hundred-MB transcript
//! into memory.

use crate::model::{Session, Source};
use anyhow::Result;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};

/// Absolute ceiling on messages read in one call, regardless of offset/limit.
const HARD_CAP: usize = 50_000;

pub struct Message {
    pub role: String,
    pub text: String,
    pub ts: Option<String>,
}

pub struct Page {
    pub messages: Vec<Message>,
    /// Exact total when the whole transcript fit under the read cap; `None` when
    /// the read was capped (more messages exist than were counted).
    pub total: Option<usize>,
    pub next_offset: Option<usize>,
}

/// Read one page of a session's transcript. `offset` is a message index.
pub fn read(session: &Session, offset: usize, limit: usize) -> Result<Page> {
    // Read at most one past the page so we can tell whether a next page exists,
    // bounded by HARD_CAP. saturating_* avoids overflow on hostile inputs.
    let cap = offset.saturating_add(limit).saturating_add(1).min(HARD_CAP);
    let collected = match session.source {
        Source::Claude => read_claude(session, cap)?,
        Source::Codex => read_codex(session, cap)?,
        Source::Gemini => read_gemini(session, cap)?,
        Source::Opencode => read_opencode(session, cap)?,
        Source::Shell => read_shell(session, cap)?,
    };
    let capped = collected.len() >= cap;
    let total = if capped { None } else { Some(collected.len()) };
    let messages: Vec<Message> = collected.into_iter().skip(offset).take(limit).collect();
    let next_offset = if capped && !messages.is_empty() {
        Some(offset.saturating_add(limit))
    } else {
        None
    };
    Ok(Page {
        messages,
        total,
        next_offset,
    })
}

fn read_claude(s: &Session, cap: usize) -> Result<Vec<Message>> {
    let f = File::open(&s.file_path)?;
    let mut out = Vec::new();
    for line in BufReader::new(f).lines().map_while(Result::ok) {
        if line.is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let ts = v
            .get("timestamp")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string());
        match v.get("type").and_then(|x| x.as_str()).unwrap_or("") {
            "user" => {
                if let Some(text) = claude_user_text(&v) {
                    out.push(Message {
                        role: "user".into(),
                        text,
                        ts,
                    });
                }
            }
            "assistant" => {
                let text = claude_blocks_text(v.pointer("/message/content"));
                if !text.is_empty() {
                    out.push(Message {
                        role: "assistant".into(),
                        text,
                        ts,
                    });
                }
            }
            _ => {}
        }
        if out.len() >= cap {
            break;
        }
    }
    Ok(out)
}

fn claude_user_text(v: &Value) -> Option<String> {
    if v.get("isMeta").and_then(|x| x.as_bool()) == Some(true) {
        return None;
    }
    let content = v.pointer("/message/content")?;
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(_) => claude_blocks_text(Some(content)),
        _ => return None,
    };
    let text = text.trim();
    if text.is_empty()
        || text.starts_with("<command-")
        || text.starts_with("<local-command")
        || text.starts_with("<system-reminder")
    {
        return None;
    }
    Some(text.to_string())
}

/// Join the `text` blocks of a Claude content array (skipping thinking/tool_use).
fn claude_blocks_text(content: Option<&Value>) -> String {
    let arr = match content.and_then(|c| c.as_array()) {
        Some(a) => a,
        None => return String::new(),
    };
    let mut buf = String::new();
    for el in arr {
        if el.get("type").and_then(|x| x.as_str()) == Some("text") {
            if let Some(t) = el.get("text").and_then(|x| x.as_str()) {
                buf.push_str(t);
                buf.push('\n');
            }
        }
    }
    buf.trim().to_string()
}

fn read_codex(s: &Session, cap: usize) -> Result<Vec<Message>> {
    let f = File::open(&s.file_path)?;
    let mut out = Vec::new();
    for line in BufReader::new(f).lines().map_while(Result::ok) {
        if line.is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("type").and_then(|x| x.as_str()) != Some("event_msg") {
            continue;
        }
        let ts = v
            .get("timestamp")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string());
        let p = match v.get("payload") {
            Some(p) => p,
            None => continue,
        };
        let role = match p.get("type").and_then(|x| x.as_str()).unwrap_or("") {
            "user_message" => "user",
            "agent_message" => "assistant",
            _ => continue,
        };
        let text = p
            .get("message")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim();
        if text.is_empty() || (role == "user" && codex_injected(text)) {
            continue;
        }
        out.push(Message {
            role: role.into(),
            text: text.to_string(),
            ts,
        });
        if out.len() >= cap {
            break;
        }
    }
    Ok(out)
}

fn codex_injected(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with('<') || t.starts_with("IMPORTANT: Do NOT read") || t.starts_with("# ")
}

fn read_gemini(s: &Session, cap: usize) -> Result<Vec<Message>> {
    // The chats file is one session; collect_session handles both storage
    // formats (top-level lines and $set/$push snapshots), deduped by id.
    let f = File::open(&s.file_path)?;
    let (_, _, msgs) = crate::scan::gemini::collect_session(BufReader::new(f));
    let mut out = Vec::new();
    for m in msgs {
        if m.text.is_empty() || m.text.starts_with("<session_context>") {
            continue;
        }
        let role = if m.role == "gemini" {
            "assistant"
        } else {
            "user"
        };
        out.push(Message {
            role: role.into(),
            text: m.text,
            ts: m.ts,
        });
        if out.len() >= cap {
            break;
        }
    }
    Ok(out)
}

fn read_opencode(s: &Session, cap: usize) -> Result<Vec<Message>> {
    let conn = Connection::open_with_flags(&s.file_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.busy_timeout(std::time::Duration::from_secs(3))?;
    let mut stmt = conn.prepare(
        "SELECT m.data, p.data FROM message m JOIN part p ON p.message_id = m.id
         WHERE m.session_id = ?1 ORDER BY m.time_created, p.time_created LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![s.id, cap as i64], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (mdata, pdata) = row?;
        let pv: Value = match serde_json::from_str(&pdata) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if pv.get("type").and_then(|x| x.as_str()) != Some("text") {
            continue;
        }
        let text = pv.get("text").and_then(|x| x.as_str()).unwrap_or("").trim();
        if text.is_empty() {
            continue;
        }
        let role = serde_json::from_str::<Value>(&mdata)
            .ok()
            .and_then(|m| {
                m.get("role")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "message".into());
        out.push(Message {
            role,
            text: text.to_string(),
            ts: None,
        });
    }
    Ok(out)
}

fn read_shell(s: &Session, cap: usize) -> Result<Vec<Message>> {
    let f = File::open(&s.file_path)?;
    let mut out = Vec::new();
    for line in BufReader::new(f).lines().map_while(Result::ok) {
        let mut parts = line.splitn(3, '\t');
        let ts = parts.next().unwrap_or("");
        let dir = parts.next().unwrap_or("");
        let cmd = parts.next().unwrap_or("").trim();
        if dir != s.cwd || cmd.is_empty() {
            continue;
        }
        out.push(Message {
            role: "command".into(),
            text: cmd.to_string(),
            ts: Some(ts.to_string()),
        });
        if out.len() >= cap {
            break;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Session;

    fn sess(source: Source, id: &str, file: &str, cwd: &str) -> Session {
        Session {
            id: id.into(),
            source,
            file_path: file.into(),
            cwd: cwd.into(),
            title: "t".into(),
            first_prompt: String::new(),
            last_prompt: String::new(),
            model: None,
            git_branch: None,
            started_at: 0,
            updated_at: 0,
            msg_count: 0,
            bypass: false,
        }
    }

    #[test]
    fn reads_and_paginates_claude() {
        let dir = std::env::temp_dir().join(format!("termem-tx-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("s.jsonl");
        std::fs::write(
            &f,
            "{\"type\":\"user\",\"cwd\":\"/w\",\"timestamp\":\"2026-01-01T00:00:00.000Z\",\"message\":{\"content\":\"hello\"}}\n{\"type\":\"assistant\",\"timestamp\":\"2026-01-01T00:00:01.000Z\",\"message\":{\"content\":[{\"type\":\"thinking\",\"thinking\":\"hmm\"},{\"type\":\"text\",\"text\":\"hi there\"}]}}\n{\"type\":\"user\",\"message\":{\"content\":\"second\"}}\n",
        )
        .unwrap();
        let s = sess(Source::Claude, "s", f.to_str().unwrap(), "/w");

        // Whole transcript fits under the cap: exact total, no next page.
        let full = read(&s, 0, 10).unwrap();
        assert_eq!(full.total, Some(3));
        assert_eq!(full.messages.len(), 3);
        assert_eq!(full.next_offset, None);
        assert_eq!(full.messages[0].text, "hello");
        assert_eq!(full.messages[1].role, "assistant");
        assert_eq!(full.messages[1].text, "hi there"); // thinking skipped
        assert_eq!(full.messages[2].text, "second");

        // Paginated.
        let p0 = read(&s, 0, 2).unwrap();
        assert_eq!(p0.messages.len(), 2);
        assert_eq!(p0.next_offset, Some(2));
        let p1 = read(&s, 2, 2).unwrap();
        assert_eq!(p1.messages.len(), 1);
        assert_eq!(p1.next_offset, None);

        // Hostile pagination args must not panic.
        let huge = read(&s, usize::MAX, usize::MAX).unwrap();
        assert!(huge.messages.is_empty());
        assert_eq!(huge.next_offset, None);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
