//! End-to-end test of the `termem mcp` stdio server: spawn the real binary,
//! drive it with JSON-RPC, and assert the protocol + the save_summary -> recall
//! "cached" compounding loop. Runs hermetically by pointing every source at an
//! empty/disabled location via env, so it does not touch real session data.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static RUN: AtomicU64 = AtomicU64::new(0);

fn run_mcp(requests: &[&str]) -> Vec<serde_json::Value> {
    // Unique per call so parallel tests never share a SQLite file.
    let n = RUN.fetch_add(1, Ordering::Relaxed);
    let db = std::env::temp_dir().join(format!("termem-mcp-it-{}-{}.db", std::process::id(), n));
    let _ = std::fs::remove_file(&db);

    let mut child = Command::new(env!("CARGO_BIN_EXE_termem"))
        .arg("mcp")
        .env("TERMEM_DB", &db)
        // Disable every real source: hermetic, fast, no real-data egress.
        .env("TERMEM_CLAUDE_DIR", "")
        .env("TERMEM_CODEX_DIR", "")
        .env("TERMEM_GEMINI_DIR", "")
        .env("TERMEM_OPENCODE_DB", "")
        .env("TERMEM_SHELL_DIR", "")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn termem mcp");

    {
        let mut stdin = child.stdin.take().unwrap();
        for r in requests {
            writeln!(stdin, "{r}").unwrap();
        }
        stdin.flush().unwrap();
        // drop stdin -> EOF -> server loop ends
    }

    let stdout = child.stdout.take().unwrap();
    let mut out = Vec::new();
    for line in BufReader::new(stdout).lines() {
        let line = line.unwrap();
        if line.trim().is_empty() {
            continue;
        }
        out.push(serde_json::from_str(&line).expect("valid json-rpc line"));
    }
    let _ = child.wait();
    let _ = std::fs::remove_file(&db);
    out
}

/// Decode a tools/call result's text content block into JSON.
fn tool_payload(resp: &serde_json::Value) -> serde_json::Value {
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    serde_json::from_str(text).expect("tool payload json")
}

#[test]
fn initialize_and_tools_list() {
    let resps = run_mcp(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{}}}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
    ]);
    // The notification produces no response.
    assert_eq!(resps.len(), 2);
    assert_eq!(resps[0]["result"]["serverInfo"]["name"], "termem");
    assert_eq!(resps[0]["result"]["protocolVersion"], "2025-06-18");

    let names: Vec<String> = resps[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    for want in ["search", "recall", "get_session", "save_summary", "stats"] {
        assert!(names.contains(&want.to_string()), "missing tool {want}");
    }
}

#[test]
fn save_summary_then_recall_is_cached() {
    // With all sources disabled, no sessions exist, but save_summary still
    // writes to the durable store and the protocol round-trips cleanly.
    let resps = run_mcp(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{}}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"save_summary","arguments":{"id":"itest-1","summary":"did a thing","unfinished":"none"}}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"stats","arguments":{"scope":"all"}}}"#,
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"recall","arguments":{"scope":"all"}}}"#,
    ]);
    assert_eq!(resps.len(), 4);

    let saved = tool_payload(&resps[1]);
    assert_eq!(saved["ok"], true);

    let stats = tool_payload(&resps[2]);
    assert_eq!(stats["sessions"], 0, "no sources -> no sessions");

    let recall = tool_payload(&resps[3]);
    assert!(recall["sessions"].as_array().unwrap().is_empty());
    assert_eq!(recall["scope"], "all");
}

#[test]
fn unknown_tool_is_an_error_result_not_a_crash() {
    let resps = run_mcp(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"nope","arguments":{}}}"#,
    ]);
    assert_eq!(resps.len(), 2);
    assert_eq!(resps[1]["result"]["isError"], true);
}
