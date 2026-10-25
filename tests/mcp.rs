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

// Explicit board-only mode uses the same configured database, but never indexes sessions.
// All paths below are disposable synthetic fixtures; HOME and the live store are untouched.
fn board_fixture(tag: &str) -> std::path::PathBuf {
    let n = RUN.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("termem-board-{tag}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture_command(dir: &std::path::Path, board_only: bool) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_termem"));
    cmd.arg("mcp");
    if board_only {
        cmd.arg("--board-only");
    }
    cmd.env("TERMEM_DB", dir.join("index.db"));
    for (key, name) in [
        ("TERMEM_CLAUDE_DIR", "claude"),
        ("TERMEM_CODEX_DIR", "codex"),
        ("TERMEM_GEMINI_DIR", "gemini"),
        ("TERMEM_OPENCODE_DB", "opencode.db"),
        ("TERMEM_SHELL_DIR", "shell"),
    ] {
        cmd.env(key, dir.join(name));
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

fn fixture_run(
    dir: &std::path::Path,
    board_only: bool,
    requests: &[serde_json::Value],
) -> std::process::Output {
    let mut child = fixture_command(dir, board_only).spawn().unwrap();
    {
        let mut input = child.stdin.take().unwrap();
        for req in requests {
            writeln!(input, "{req}").unwrap();
        }
    }
    child.wait_with_output().unwrap()
}

fn responses(output: &std::process::Output) -> Vec<serde_json::Value> {
    assert!(
        output.status.success(),
        "server refused: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone())
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn request(id: i64, method: &str, params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"jsonrpc":"2.0", "id":id, "method":method, "params":params})
}

#[test]
fn board_only_advertises_two_tools_and_refuses_all_session_operations() {
    use serde_json::json;
    let dir = board_fixture("protocol");
    let mut req = vec![
        request(1, "initialize", json!({})),
        request(2, "tools/list", json!({})),
        request(
            3,
            "tools/call",
            json!({"name":"post","arguments":{"dir":"/synthetic/project","body":"synthetic board marker"}}),
        ),
        request(
            4,
            "tools/call",
            json!({"name":"read_board","arguments":{"dir":"/synthetic/project","scope":"here"}}),
        ),
    ];
    let forbidden = [
        "search",
        "recall",
        "get_session",
        "save_summary",
        "resolve",
        "stats",
        "not-a-tool",
    ];
    for (i, name) in forbidden.iter().enumerate() {
        req.push(request(10+i as i64,"tools/call",json!({"name":name,"arguments":{"id":"synthetic-cache","summary":"must not be stored"}})));
    }
    let out = fixture_run(&dir, true, &req);
    let r = responses(&out);
    assert_eq!(
        r[0]["result"]["capabilities"]["experimental"]["termem/board-only"],
        true
    );
    let names: Vec<_> = r[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["post", "read_board"]);
    assert_eq!(tool_payload(&r[2])["ok"], true);
    assert_eq!(
        tool_payload(&r[3])["posts"][0]["body"],
        "synthetic board marker"
    );
    for (i, _) in forbidden.iter().enumerate() {
        assert_eq!(r[4 + i]["result"]["isError"], true);
        assert_eq!(
            r[4 + i]["result"]["content"][0]["text"],
            "TERMEM_BOARD_ONLY_TOOL_UNAVAILABLE"
        );
    }
    // A second process sees the same durable board, not a new/shadow store.
    let again = fixture_run(
        &dir,
        true,
        &[request(
            1,
            "tools/call",
            json!({"name":"read_board","arguments":{"dir":"/synthetic/project"}}),
        )],
    );
    assert_eq!(
        tool_payload(&responses(&again)[0])["posts"][0]["body"],
        "synthetic board marker"
    );
}

#[test]
fn board_only_preserves_old_session_schema_and_all_nonboard_rows() {
    use rusqlite::Connection;
    use serde_json::json;
    let dir = board_fixture("old-schema");
    let db = dir.join("index.db");
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch("PRAGMA user_version=1; CREATE TABLE sessions (payload TEXT); INSERT INTO sessions VALUES ('synthetic cached session'); CREATE TABLE content_fts (payload TEXT); INSERT INTO content_fts VALUES ('synthetic cached search body'); CREATE TABLE summaries (payload TEXT); INSERT INTO summaries VALUES ('synthetic durable summary'); CREATE TABLE path_remap (payload TEXT); INSERT INTO path_remap VALUES ('synthetic remap');").unwrap();
    let tables = ["sessions", "content_fts", "summaries", "path_remap"];
    let snapshot = |conn: &Connection| -> Vec<(String, String)> {
        tables
            .iter()
            .map(|name| {
                let sql: String = conn
                    .query_row("SELECT sql FROM sqlite_master WHERE name=?1", [name], |r| {
                        r.get(0)
                    })
                    .unwrap_or_else(|e| format!("query failed: {e}"));
                let payload: String = conn
                    .query_row(&format!("SELECT payload FROM {name}"), [], |r| r.get(0))
                    .unwrap_or_else(|e| format!("query failed: {e}"));
                (sql, payload)
            })
            .collect()
    };
    let before = snapshot(&conn);
    drop(conn);
    let out = fixture_run(
        &dir,
        true,
        &[
            request(1, "initialize", json!({})),
            request(
                2,
                "tools/call",
                json!({"name":"post","arguments":{"body":"board only"}}),
            ),
        ],
    );
    assert_eq!(responses(&out).len(), 2);
    let conn = Connection::open(&db).unwrap();
    assert_eq!(snapshot(&conn), before);
    assert_eq!(
        conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM board", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn board_only_never_refreshes_current_cache_on_startup_or_rejected_call() {
    use serde_json::json;
    use termem::{index::Index, scan::ScanRoots};
    let dir = board_fixture("current-cache");
    let source = dir.join("claude");
    std::fs::create_dir_all(&source).unwrap();
    let file = source.join("synthetic.jsonl");
    std::fs::write(&file,"{\"type\":\"user\",\"cwd\":\"/synthetic/project\",\"timestamp\":\"2026-01-01T00:00:00.000Z\",\"message\":{\"content\":\"synthetic searchable cache\"}}\n").unwrap();
    let roots = ScanRoots {
        claude: Some(source),
        codex: None,
        gemini: None,
        opencode: None,
        shell: None,
    };
    let mut index = Index::open_with_roots(&dir.join("index.db"), roots).unwrap();
    assert_eq!(index.refresh().unwrap().parsed, 1);
    drop(index);
    std::fs::remove_file(file).unwrap(); // A refresh would now delete the cached row and its FTS body.
    let out = fixture_run(
        &dir,
        true,
        &[
            request(1, "initialize", json!({})),
            request(
                2,
                "tools/call",
                json!({"name":"stats","arguments":{"scope":"all"}}),
            ),
        ],
    );
    let r = responses(&out);
    assert_eq!(r[1]["result"]["isError"], true);
    let conn = rusqlite::Connection::open(dir.join("index.db")).unwrap();
    assert_eq!(
        conn.query_row("SELECT count(*) FROM sessions", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM content_fts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn normal_mcp_still_refreshes_and_searches_synthetic_sessions() {
    use serde_json::json;
    let dir = board_fixture("normal");
    let source = dir.join("claude");
    std::fs::create_dir_all(&source).unwrap();
    let mut child = fixture_command(&dir, false).spawn().unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    writeln!(input, "{}", request(1, "initialize", json!({}))).unwrap();
    input.flush().unwrap();
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    let initialized: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert!(initialized["result"]["capabilities"]
        .get("experimental")
        .is_none());
    // Created after initialize: the normal read-tool refresh must discover it.
    std::fs::write(source.join("synthetic.jsonl"),"{\"type\":\"user\",\"cwd\":\"/synthetic/project\",\"timestamp\":\"2026-01-01T00:00:00.000Z\",\"message\":{\"content\":\"portabletermemcanary\"}}\n").unwrap();
    writeln!(
        input,
        "{}",
        request(
            2,
            "tools/call",
            json!({"name":"stats","arguments":{"scope":"all"}})
        )
    )
    .unwrap();
    input.flush().unwrap();
    line.clear();
    output.read_line(&mut line).unwrap();
    let stats: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(tool_payload(&stats)["sessions"], 1);
    writeln!(
        input,
        "{}",
        request(
            3,
            "tools/call",
            json!({"name":"search","arguments":{"scope":"all","query":"portabletermemcanary"}})
        )
    )
    .unwrap();
    input.flush().unwrap();
    line.clear();
    output.read_line(&mut line).unwrap();
    let search: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(
        tool_payload(&search)["results"].as_array().unwrap().len(),
        1
    );
    drop(input);
    assert!(child.wait().unwrap().success());
}

#[test]
fn board_only_errors_are_constant_and_do_not_echo_paths_or_tool_names() {
    use serde_json::json;
    let dir = board_fixture("bounded-errors");
    let tool_name = "synthetic-sensitive-name".repeat(500);
    let out = fixture_run(
        &dir,
        true,
        &[
            request(1, "tools/call", json!({"name":"post","arguments":{}})),
            request(
                2,
                "tools/call",
                json!({"name":tool_name,"arguments":{"path":"synthetic-private-path"}}),
            ),
        ],
    );
    let r = responses(&out);
    assert_eq!(
        r[0]["result"]["content"][0]["text"],
        "TERMEM_BOARD_OPERATION_FAILED"
    );
    assert_eq!(
        r[1]["result"]["content"][0]["text"],
        "TERMEM_BOARD_ONLY_TOOL_UNAVAILABLE"
    );
    assert!(!String::from_utf8_lossy(&out.stdout).contains("synthetic-sensitive-name"));
    let bad = board_fixture("bad-store");
    std::fs::create_dir(bad.join("index.db")).unwrap();
    let out = fixture_run(&bad, true, &[request(1, "initialize", json!({}))]);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert_eq!(
        String::from_utf8(out.stderr).unwrap().trim(),
        "Error: TERMEM_BOARD_STORE_UNAVAILABLE"
    );
}
