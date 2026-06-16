//! termem MCP server over stdio. The single interface every agent uses to read
//! and write terminal memory. No model calls, no network: retrieval + storage
//! only. Newline-delimited JSON-RPC 2.0; logs go to stderr.

use crate::index::Index;
use crate::model::{Session, Source};
use crate::query::{self, Scope};
use crate::{board, memory, transcript};
use anyhow::Result;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

const PROTOCOL_VERSION: &str = "2025-06-18";

pub fn serve() -> Result<()> {
    let mut index = Index::open_default()?;
    let _ = index.refresh(); // best-effort; still serve if a scan hiccups

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(resp) = handle(&mut index, &req) {
            let s = serde_json::to_string(&resp).unwrap_or_default();
            out.write_all(s.as_bytes())?;
            out.write_all(b"\n")?;
            out.flush()?;
        }
    }
    Ok(())
}

fn handle(index: &mut Index, req: &Value) -> Option<Value> {
    let id = req.get("id").cloned();
    match req.get("method").and_then(|m| m.as_str()).unwrap_or("") {
        "initialize" => Some(ok(
            id,
            json!({
                "protocolVersion": req.pointer("/params/protocolVersion")
                    .and_then(|v| v.as_str()).unwrap_or(PROTOCOL_VERSION),
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "termem", "version": env!("CARGO_PKG_VERSION")},
            }),
        )),
        "notifications/initialized" | "notifications/cancelled" => None,
        "ping" => Some(ok(id, json!({}))),
        "tools/list" => Some(ok(id, json!({ "tools": tool_list() }))),
        "tools/call" => Some(handle_call(index, id, req)),
        _ if id.is_some() => Some(err(id, -32601, "Method not found")),
        _ => None,
    }
}

fn ok(id: Option<Value>, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "result": result})
}

fn err(id: Option<Value>, code: i64, msg: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "error": {"code": code, "message": msg}})
}

fn tool_ok(id: Option<Value>, value: Value) -> Value {
    let text = serde_json::to_string_pretty(&value).unwrap_or_default();
    ok(
        id,
        json!({"content": [{"type": "text", "text": text}], "isError": false}),
    )
}

fn tool_err(id: Option<Value>, msg: &str) -> Value {
    ok(
        id,
        json!({"content": [{"type": "text", "text": msg}], "isError": true}),
    )
}

fn handle_call(index: &mut Index, id: Option<Value>, req: &Value) -> Value {
    let name = req
        .pointer("/params/name")
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .to_string();
    let args = req
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or(json!({}));

    // Keep the index fresh for read tools (cheap incremental refresh).
    if matches!(name.as_str(), "search" | "recall" | "stats" | "get_session") {
        let _ = index.refresh();
    }

    let result = match name.as_str() {
        "search" => tool_search(index, &args),
        "recall" => tool_recall(index, &args),
        "get_session" => tool_get_session(index, &args),
        "save_summary" => tool_save_summary(index, &args),
        "post" => tool_post(index, &args),
        "read_board" => tool_read_board(index, &args),
        "stats" => tool_stats(index, &args),
        other => return tool_err(id, &format!("unknown tool: {other}")),
    };
    match result {
        Ok(v) => tool_ok(id, v),
        Err(e) => tool_err(id, &format!("{e}")),
    }
}

// ---- arg helpers ----

fn arg_str<'a>(args: &'a Value, k: &str) -> Option<&'a str> {
    args.get(k).and_then(|v| v.as_str())
}

fn arg_i64(args: &Value, k: &str, default: i64) -> i64 {
    args.get(k).and_then(|v| v.as_i64()).unwrap_or(default)
}

fn cwd_arg(args: &Value) -> String {
    arg_str(args, "dir")
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default()
        })
}

fn scope_arg(args: &Value, default: Scope) -> Scope {
    match arg_str(args, "scope") {
        Some("here") => Scope::Here,
        Some("tree") => Scope::Subtree,
        Some("all") => Scope::All,
        _ => default,
    }
}

fn scope_name(s: Scope) -> &'static str {
    match s {
        Scope::Here => "here",
        Scope::Subtree => "tree",
        Scope::All => "all",
    }
}

fn sources_arg(args: &Value) -> Vec<Source> {
    match arg_str(args, "source") {
        Some("all") | None => vec![],
        Some(s) => Source::from_tag(s).into_iter().collect(),
    }
}

fn iso(ms: i64) -> String {
    use chrono::TimeZone;
    chrono::Utc
        .timestamp_millis_opt(ms)
        .single()
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

// ---- tools ----

fn tool_search(index: &Index, args: &Value) -> Result<Value> {
    let query_s = arg_str(args, "query").unwrap_or("").to_string();
    let dir = cwd_arg(args);
    let scope = scope_arg(args, Scope::Subtree);
    let sources = sources_arg(args);
    let limit = arg_i64(args, "limit", 20);
    let sessions = query::search(index.conn(), &query_s, &dir, scope, &sources, limit)?;
    let results: Vec<Value> = sessions
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "source": s.source.as_str(),
                "title": s.title,
                "dir": s.cwd,
                "started_at": iso(s.started_at),
                "last_active": iso(s.updated_at),
                "messages": s.msg_count,
                "has_summary": memory::has_summary(index.conn(), &s.id),
                "score": 1.0,
            })
        })
        .collect();
    Ok(json!({"query": query_s, "scope": scope_name(scope), "dir": dir, "results": results}))
}

fn tool_recall(index: &Index, args: &Value) -> Result<Value> {
    let dir = cwd_arg(args);
    let scope = scope_arg(args, Scope::Subtree);
    let query_s = arg_str(args, "query");
    let max = arg_i64(args, "max_sessions", 5);
    let rows = memory::recall(index.conn(), &dir, scope, query_s, max)?;

    let mut needs = 0;
    let sessions: Vec<Value> = rows
        .iter()
        .map(|r| {
            let mut obj = json!({
                "id": r.session.id,
                "source": r.session.source.as_str(),
                "dir": r.session.cwd,
                "last_active": iso(r.session.updated_at),
                "summary": r.summary,
                "unfinished": r.unfinished,
                "summary_state": r.state.as_str(),
            });
            if r.state != memory::SummaryState::Cached {
                needs += 1;
                if let Ok(head) = head_for(&r.session) {
                    if let Some(map) = obj.as_object_mut() {
                        map.insert("head".into(), json!(head));
                    }
                }
            }
            obj
        })
        .collect();

    let note = if needs > 0 {
        format!(
            "{needs} session(s) have no current summary. Read each via get_session and \
             store a primer via save_summary so every agent benefits next time."
        )
    } else {
        "all sessions in scope have a cached summary".to_string()
    };
    Ok(json!({"dir": dir, "scope": scope_name(scope), "sessions": sessions, "note": note}))
}

fn head_for(s: &Session) -> Result<String> {
    let page = transcript::read(s, 0, 8)?;
    let mut buf = String::new();
    for m in page.messages {
        let snippet: String = m.text.chars().take(280).collect();
        buf.push_str(&format!("[{}] {}\n", m.role, snippet));
    }
    Ok(buf.trim().to_string())
}

fn tool_get_session(index: &Index, args: &Value) -> Result<Value> {
    let id = arg_str(args, "id").ok_or_else(|| anyhow::anyhow!("id is required"))?;
    let session = match query::find_one(index.conn(), id)? {
        Some(s) => s,
        None => return Ok(json!({"error": format!("no session matching {id}")})),
    };
    let offset: usize = arg_str(args, "cursor")
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let limit = arg_i64(args, "max_messages", 50).clamp(1, 1000) as usize;
    let page = transcript::read(&session, offset, limit)?;
    let messages: Vec<Value> = page
        .messages
        .iter()
        .map(|m| json!({"role": m.role, "text": m.text, "ts": m.ts}))
        .collect();
    let chars: usize = page.messages.iter().map(|m| m.text.len()).sum();
    Ok(json!({
        "id": session.id,
        "source": session.source.as_str(),
        "dir": session.cwd,
        "title": session.title,
        "total_messages": page.total,
        "messages": messages,
        "next_cursor": page.next_offset.map(|o| o.to_string()),
        "approx_tokens": chars / 4,
    }))
}

fn tool_save_summary(index: &Index, args: &Value) -> Result<Value> {
    let id = arg_str(args, "id").ok_or_else(|| anyhow::anyhow!("id is required"))?;
    let summary = arg_str(args, "summary").ok_or_else(|| anyhow::anyhow!("summary is required"))?;
    let unfinished = arg_str(args, "unfinished");
    let tags: Vec<String> = args
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let known = memory::save_summary(index.conn(), id, summary, unfinished, &tags)?;
    Ok(json!({"ok": true, "id": id, "known_session": known}))
}

fn tool_post(index: &Index, args: &Value) -> Result<Value> {
    let body = arg_str(args, "body").ok_or_else(|| anyhow::anyhow!("body is required"))?;
    let dir = cwd_arg(args);
    let kind = arg_str(args, "kind").unwrap_or("note");
    let author = arg_str(args, "author");
    let id = board::post(index.conn(), &dir, author, kind, body)?;
    Ok(json!({"ok": true, "id": id, "dir": dir, "kind": kind}))
}

fn tool_read_board(index: &Index, args: &Value) -> Result<Value> {
    let dir = cwd_arg(args);
    let scope = scope_arg(args, Scope::Subtree);
    let since = arg_i64(args, "since", 0);
    let kind = arg_str(args, "kind");
    let limit = arg_i64(args, "limit", 50);
    let posts = board::read(index.conn(), &dir, scope, since, kind, limit)?;
    // Newest first; the latest timestamp is the cursor to pass back as `since`.
    let cursor = posts.iter().map(|p| p.created_at).max().unwrap_or(since);
    let items: Vec<Value> = posts
        .iter()
        .map(|p| {
            json!({
                "id": p.id,
                "author": p.author,
                "kind": p.kind,
                "body": p.body,
                "dir": p.cwd,
                "at": iso(p.created_at),
                "ts": p.created_at,
            })
        })
        .collect();
    Ok(json!({
        "dir": dir,
        "scope": scope_name(scope),
        "posts": items,
        "cursor": cursor,
        "note": "pass `cursor` back as `since` next time to read only newer posts",
    }))
}

fn tool_stats(index: &Index, args: &Value) -> Result<Value> {
    let dir = cwd_arg(args);
    let scope = scope_arg(args, Scope::Subtree);
    let sessions = query::query(index.conn(), &dir, scope, &[], None, 1_000_000)?;
    let total = sessions.len();
    let messages: i64 = sessions.iter().map(|s| s.msg_count).sum();
    let mut by_source: std::collections::BTreeMap<&str, i64> = std::collections::BTreeMap::new();
    for s in &sessions {
        *by_source.entry(s.source.as_str()).or_insert(0) += 1;
    }
    Ok(json!({
        "dir": dir,
        "scope": scope_name(scope),
        "sessions": total,
        "messages": messages,
        "by_source": by_source,
    }))
}

// ---- tool schemas ----

fn tool_list() -> Value {
    json!([
        {
            "name": "search",
            "description": "Find past sessions by matching the query against title, first prompt, and path. Directory-scoped by default; use for 'find the session where X happened'.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "matched against title/prompt/path"},
                    "source": {"type": "string", "enum": ["claude","codex","gemini","opencode","shell","all"], "description": "filter by tool (default all)"},
                    "scope": {"type": "string", "enum": ["here","tree","all"], "description": "here=exact dir, tree=dir+subfolders (default), all=whole machine"},
                    "dir": {"type": "string", "description": "anchor for here/tree (default cwd)"},
                    "limit": {"type": "integer", "description": "max results (default 20)"}
                },
                "required": ["query"]
            }
        },
        {
            "name": "recall",
            "description": "Orient on entering a directory: returns distilled primers for recent sessions in scope, each marked cached/needs_summary/stale. Call this first when prior context in this directory would help.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "dir": {"type": "string", "description": "directory to orient around (default cwd)"},
                    "query": {"type": "string", "description": "optional focus, e.g. 'auth flow'"},
                    "scope": {"type": "string", "enum": ["here","tree","all"], "description": "all requires the user to ask first; it surfaces unrelated repos"},
                    "max_sessions": {"type": "integer", "description": "how many sessions to fold in (default 5)"}
                }
            }
        },
        {
            "name": "get_session",
            "description": "Fetch the raw messages of one session (paginated) so you can distil a summary or answer a detailed question.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "session id"},
                    "cursor": {"type": "string", "description": "pagination cursor from a prior call"},
                    "max_messages": {"type": "integer", "description": "page size (default 50)"}
                },
                "required": ["id"]
            }
        },
        {
            "name": "save_summary",
            "description": "Store an agent-authored primer for a session in termem's own store (never the source file). This is how memory is built for every future agent.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "session being summarised"},
                    "summary": {"type": "string", "description": "the distilled primer"},
                    "unfinished": {"type": "string", "description": "what was left open (high value for resumption)"},
                    "tags": {"type": "array", "items": {"type": "string"}}
                },
                "required": ["id", "summary"]
            }
        },
        {
            "name": "post",
            "description": "Pin a short note to this directory's shared board so other agent sessions working here can read it later. Use for coordination state: a claim ('refactoring auth, leave it'), a handoff ('migration written but unrun'), or a fact worth surfacing to the next session. Async only: posting does not interrupt another session; it is read when that session calls read_board.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "body": {"type": "string", "description": "the note (keep it short and factual)"},
                    "kind": {"type": "string", "description": "note | task | claim | done (free-form; default note)"},
                    "author": {"type": "string", "description": "who is posting, e.g. agent/session label (optional)"},
                    "dir": {"type": "string", "description": "board directory (default cwd); post and read from the same anchor, usually the repo root"}
                },
                "required": ["body"]
            }
        },
        {
            "name": "read_board",
            "description": "Read notes other sessions pinned to this directory's board. Call on entering a directory, alongside recall, to pick up coordination state. Returns a `cursor`; pass it back as `since` next time to get only newer posts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "dir": {"type": "string", "description": "board directory (default cwd)"},
                    "scope": {"type": "string", "enum": ["here","tree","all"], "description": "here=exact dir, tree=dir+subfolders (default), all=whole machine"},
                    "since": {"type": "integer", "description": "epoch-ms cursor from a prior read; only posts newer than this are returned (default 0 = all)"},
                    "kind": {"type": "string", "description": "filter to one kind, e.g. 'claim' (optional)"},
                    "limit": {"type": "integer", "description": "max posts (default 50)"}
                }
            }
        },
        {
            "name": "stats",
            "description": "Read-only aggregation over the index: session and message counts per directory and source. Pure local arithmetic.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "dir": {"type": "string"},
                    "scope": {"type": "string", "enum": ["here","tree","all"]}
                }
            }
        }
    ])
}
