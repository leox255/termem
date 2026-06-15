//! Integration tests against the real session data on this machine. Each test
//! self-skips if the corresponding tool's data is absent, so the suite stays
//! green on a fresh checkout while still proving the acceptance criteria here.

use std::path::PathBuf;
use termem::index::Index;
use termem::model::Source;
use termem::query::{query, search, Scope};
use termem::scan::ScanRoots;

fn temp_db(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("termem-it-{}-{}.db", tag, std::process::id()))
}

fn home() -> PathBuf {
    // HOME on Unix, USERPROFILE on Windows. The tests below self-skip when the
    // expected data dir is absent, so this just needs to resolve without
    // panicking on either platform.
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .expect("home dir")
}

#[test]
fn indexes_and_finds_this_build_session() {
    let proj = home().join(".claude/projects/-Users-amosff-ai-apps-termem");
    if !proj.exists() {
        eprintln!("skip: no claude data for this project");
        return;
    }
    let db = temp_db("claude");
    let _ = std::fs::remove_file(&db);
    let mut idx = Index::open(&db).unwrap();
    let stats = idx.refresh().unwrap();
    assert!(stats.total > 0, "expected to discover session files");

    let res = query(
        idx.conn(),
        "/Users/amosff/ai/apps/termem",
        Scope::Subtree,
        &[],
        None,
        500,
    )
    .unwrap();

    let me = res
        .iter()
        .find(|s| s.id == "13b4ddd9-8f86-4e4d-bd23-b4fc83d286dd");
    match me {
        Some(s) => {
            assert_eq!(s.source, Source::Claude);
            assert_eq!(
                s.title,
                "Build terminal memory system for session management"
            );
            assert_eq!(s.cwd, "/Users/amosff/ai/apps/termem");
        }
        None => panic!(
            "this build session not indexed; got {} sessions for the dir",
            res.len()
        ),
    }
    let _ = std::fs::remove_file(&db);
}

#[test]
fn incremental_refresh_only_reparses_changed_files() {
    // Use a static fixture dir (not the live $HOME tree, which is mutated by
    // any running session) so incremental behavior is deterministic.
    let dir = std::env::temp_dir().join(format!("termem-fix-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("a.jsonl"),
        "{\"type\":\"user\",\"cwd\":\"/x/a\",\"timestamp\":\"2026-01-01T00:00:00.000Z\",\"message\":{\"content\":\"hello a\"}}\n{\"type\":\"ai-title\",\"aiTitle\":\"Session A\"}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("b.jsonl"),
        "{\"type\":\"user\",\"cwd\":\"/x/b\",\"timestamp\":\"2026-01-02T00:00:00.000Z\",\"message\":{\"content\":\"hello b\"}}\n{\"type\":\"ai-title\",\"aiTitle\":\"Session B\"}\n",
    )
    .unwrap();

    let roots = ScanRoots {
        claude: Some(dir.clone()),
        codex: None,
        gemini: None,
        opencode: None,
        shell: None,
    };
    let db = temp_db("incr");
    let _ = std::fs::remove_file(&db);
    let mut idx = Index::open_with_roots(&db, roots).unwrap();

    let first = idx.refresh().unwrap();
    assert_eq!(first.total, 2);
    assert_eq!(first.parsed, 2, "first pass parses both fixtures");

    let second = idx.refresh().unwrap();
    assert_eq!(second.parsed, 0, "unchanged files are not re-parsed");
    assert_eq!(second.deleted, 0);

    // Mutate one file -> exactly one re-parse, none deleted.
    std::fs::write(
        dir.join("a.jsonl"),
        "{\"type\":\"user\",\"cwd\":\"/x/a\",\"timestamp\":\"2026-01-03T00:00:00.000Z\",\"message\":{\"content\":\"changed a\"}}\n{\"type\":\"ai-title\",\"aiTitle\":\"Session A v2\"}\n",
    )
    .unwrap();
    let third = idx.refresh().unwrap();
    assert_eq!(third.parsed, 1, "only the mutated file re-parses");

    // Title updated, found by query in its directory.
    let res = query(idx.conn(), "/x/a", Scope::Here, &[], None, 10).unwrap();
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].title, "Session A v2");

    let _ = std::fs::remove_file(&db);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shell_log_indexes_one_row_per_directory() {
    let dir = std::env::temp_dir().join(format!("termem-sh-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("s1.log"),
        "1718450000\t/tmp/projA\tls\n\
         1718450010\t/tmp/projB\tcargo build\n\
         1718450020\t/tmp/projA\tgit status\n",
    )
    .unwrap();

    let roots = ScanRoots {
        claude: None,
        codex: None,
        gemini: None,
        opencode: None,
        shell: Some(dir.clone()),
    };
    let db = temp_db("shell");
    let _ = std::fs::remove_file(&db);
    let mut idx = Index::open_with_roots(&db, roots).unwrap();

    let stats = idx.refresh().unwrap();
    assert_eq!(stats.total, 1, "one log file");
    assert_eq!(stats.parsed, 2, "two directories -> two indexed rows");

    let a = query(idx.conn(), "/tmp/projA", Scope::Here, &[], None, 10).unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].source, Source::Shell);
    assert_eq!(a[0].msg_count, 2, "two commands ran in projA");

    let b = query(idx.conn(), "/tmp/projB", Scope::Here, &[], None, 10).unwrap();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].msg_count, 1);

    let _ = std::fs::remove_file(&db);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reindex_evicts_stale_shell_directories() {
    let dir = std::env::temp_dir().join(format!("termem-shre-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("s1.log");
    std::fs::write(
        &log,
        "1718450000\t/tmp/projA\tls\n1718450010\t/tmp/projB\tcargo build\n",
    )
    .unwrap();

    let roots = ScanRoots {
        claude: None,
        codex: None,
        gemini: None,
        opencode: None,
        shell: Some(dir.clone()),
    };
    let db = temp_db("shre");
    let _ = std::fs::remove_file(&db);
    let mut idx = Index::open_with_roots(&db, roots).unwrap();
    idx.refresh().unwrap();
    assert_eq!(
        query(idx.conn(), "/tmp/projB", Scope::Here, &[], None, 10)
            .unwrap()
            .len(),
        1
    );

    // Rewrite the log: drop projB, add projC, add a command to projA. The
    // different length guarantees the (mtime, size) cache sees a change.
    std::fs::write(
        &log,
        "1718450100\t/tmp/projA\tls\n1718450110\t/tmp/projA\tgit status\n1718450120\t/tmp/projC\tmake\n",
    )
    .unwrap();
    let stats = idx.refresh().unwrap();
    assert_eq!(stats.parsed, 2, "projA and projC re-parsed");
    assert!(
        query(idx.conn(), "/tmp/projB", Scope::Here, &[], None, 10)
            .unwrap()
            .is_empty(),
        "stale projB row evicted"
    );
    assert_eq!(
        query(idx.conn(), "/tmp/projC", Scope::Here, &[], None, 10)
            .unwrap()
            .len(),
        1,
        "projC added"
    );
    let a = query(idx.conn(), "/tmp/projA", Scope::Here, &[], None, 10).unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].msg_count, 2, "projA updated to two commands");

    let _ = std::fs::remove_file(&db);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn content_fts_finds_body_matches() {
    let db = temp_db("fts");
    let _ = std::fs::remove_file(&db);
    let idx = Index::open_with_roots(
        &db,
        ScanRoots {
            claude: None,
            codex: None,
            gemini: None,
            opencode: None,
            shell: None,
        },
    )
    .unwrap();
    let conn = idx.conn();
    conn.execute(
        "INSERT INTO sessions
            (key,file_path,id,source,cwd,title,first_prompt,last_prompt,
             model,git_branch,started_at,updated_at,msg_count,file_mtime,file_size)
         VALUES ('k','/f','s1','claude','/work','A short title','first prompt','last',
                 NULL,NULL,0,1,1,0,0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO content_fts (session_id, body)
         VALUES ('s1', 'we set up nginx as a reverse proxy in front of pgbouncer')",
        [],
    )
    .unwrap();

    // A word that appears only in the body is found via the FTS index.
    let body = search(conn, "pgbouncer", "/work", Scope::Subtree, &[], 10).unwrap();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0].id, "s1");

    // A word only in the title is still found via metadata.
    assert!(search(conn, "title", "/work", Scope::Subtree, &[], 10)
        .unwrap()
        .iter()
        .any(|s| s.id == "s1"));

    // Multiple content words are AND-ed.
    assert_eq!(
        search(conn, "nginx pgbouncer", "/work", Scope::Subtree, &[], 10)
            .unwrap()
            .len(),
        1
    );

    // Absent term -> no results; other directory -> out of scope.
    assert!(search(conn, "kubernetes", "/work", Scope::Subtree, &[], 10)
        .unwrap()
        .is_empty());
    assert!(search(conn, "pgbouncer", "/other", Scope::Subtree, &[], 10)
        .unwrap()
        .is_empty());

    let _ = std::fs::remove_file(&db);
}

#[test]
fn resolves_codex_sessions_for_compstack_with_titles() {
    if !home().join(".codex/sessions").exists() {
        eprintln!("skip: no codex data");
        return;
    }
    let db = temp_db("codex");
    let _ = std::fs::remove_file(&db);
    let mut idx = Index::open(&db).unwrap();
    idx.refresh().unwrap();

    let res = query(
        idx.conn(),
        "/Users/amosff/Documents/Personal/compstack",
        Scope::Subtree,
        &[Source::Codex],
        None,
        500,
    )
    .unwrap();

    assert!(
        !res.is_empty(),
        "expected codex sessions for compstack subtree"
    );
    for s in &res {
        assert_eq!(s.source, Source::Codex);
        assert!(!s.title.trim().is_empty(), "every session needs a title");
        assert!(!s.id.trim().is_empty(), "every session needs an id");
    }
    let _ = std::fs::remove_file(&db);
}
