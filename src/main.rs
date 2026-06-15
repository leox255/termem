use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use termem::index::Index;
use termem::model::{rel_time, Session, Source};
use termem::query::{self, Scope};
use termem::{resume, shellhook, tui};

#[derive(Parser)]
#[command(
    name = "termem",
    version,
    about = "Terminal memory: index and resume Claude Code, Codex, and shell sessions by directory"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Refresh the index from disk and print stats.
    Index,
    /// List sessions for a directory.
    Ls(LsArgs),
    /// Resume a session by id or fuzzy query (default: most recent here).
    Resume(ResumeArgs),
    /// Open the interactive picker (this is also the default with no subcommand).
    Tui(TuiArgs),
    /// Print shell integration: `eval "$(termem init zsh)"`.
    Init(InitArgs),
    /// Print a one-line session count for a directory (used by the cd hook).
    Hint(HintArgs),
}

#[derive(Args, Default)]
struct ScopeArgs {
    /// Directory to scope to (default: current directory).
    #[arg(long)]
    cwd: Option<PathBuf>,
    /// Only sessions started exactly here (exclude descendants).
    #[arg(long)]
    here: bool,
    /// All sessions, ignoring directory.
    #[arg(long)]
    all: bool,
    /// Comma-separated sources to include: claude,codex,shell.
    #[arg(long)]
    source: Option<String>,
    /// Skip the incremental index refresh (use the cached index as-is).
    #[arg(long)]
    no_refresh: bool,
}

#[derive(Args)]
struct LsArgs {
    #[command(flatten)]
    scope: ScopeArgs,
    /// Filter by substring in title/prompt/cwd/id.
    #[arg(long, short)]
    search: Option<String>,
    /// Maximum rows.
    #[arg(long, default_value_t = 50)]
    limit: i64,
    /// Output JSON instead of a table.
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct TuiArgs {
    #[command(flatten)]
    scope: ScopeArgs,
}

#[derive(Args)]
struct ResumeArgs {
    /// Session id prefix or fuzzy text (empty = most recent in this directory).
    query: Option<String>,
    /// Print the `cd … && …` command instead of executing it.
    #[arg(long)]
    print: bool,
    /// Directory to scope to when no query is given.
    #[arg(long)]
    cwd: Option<PathBuf>,
    /// Skip the incremental index refresh.
    #[arg(long)]
    no_refresh: bool,
}

#[derive(Args)]
struct InitArgs {
    /// Shell to emit integration for: zsh or bash.
    shell: String,
}

#[derive(Args)]
struct HintArgs {
    /// Directory to summarize (default: current directory).
    #[arg(long)]
    cwd: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Some(Cmd::Index) => cmd_index(),
        Some(Cmd::Ls(a)) => cmd_ls(a),
        Some(Cmd::Resume(a)) => cmd_resume(a),
        Some(Cmd::Tui(a)) => cmd_tui(a.scope),
        Some(Cmd::Init(a)) => cmd_init(a),
        Some(Cmd::Hint(a)) => cmd_hint(a),
        None => cmd_tui(ScopeArgs::default()),
    }
}

fn cmd_index() -> Result<()> {
    let start = std::time::Instant::now();
    let mut idx = Index::open_default()?;
    let stats = idx.refresh()?;
    println!(
        "Indexed {} session file(s): {} parsed/updated, {} removed — {:.2}s",
        stats.total,
        stats.parsed,
        stats.deleted,
        start.elapsed().as_secs_f64()
    );
    Ok(())
}

fn cmd_ls(a: LsArgs) -> Result<()> {
    let mut idx = Index::open_default()?;
    if !a.scope.no_refresh {
        idx.refresh()?;
    }
    let cwd = resolve_cwd(&a.scope.cwd)?;
    let scope = scope_of(&a.scope);
    let sources = sources_of(&a.scope.source);
    let res = query::query(idx.conn(), &cwd, scope, &sources, a.search.as_deref(), a.limit)?;
    if a.json {
        println!("{}", serde_json::to_string_pretty(&res)?);
    } else {
        print_table(&res, &cwd, scope);
    }
    Ok(())
}

fn cmd_resume(a: ResumeArgs) -> Result<()> {
    let mut idx = Index::open_default()?;
    if !a.no_refresh {
        idx.refresh()?;
    }
    let needle = a.query.unwrap_or_default();
    let session = if needle.trim().is_empty() {
        let cwd = resolve_cwd(&a.cwd)?;
        query::query(idx.conn(), &cwd, Scope::Subtree, &[], None, 1)?
            .into_iter()
            .next()
    } else {
        query::find_one(idx.conn(), needle.trim())?
    };
    let session = match session {
        Some(s) => s,
        None => {
            eprintln!("No matching session.");
            std::process::exit(1);
        }
    };
    if a.print {
        println!("{}", resume::print_line(&session));
        Ok(())
    } else {
        // exec replaces this process; it only returns on failure.
        Err(resume::exec(&session))
    }
}

fn cmd_tui(scope: ScopeArgs) -> Result<()> {
    let mut idx = Index::open_default()?;
    if !scope.no_refresh {
        idx.refresh()?;
    }
    let cwd = resolve_cwd(&scope.cwd)?;
    let sc = scope_of(&scope);
    let sources = sources_of(&scope.source);
    let sessions = query::query(idx.conn(), &cwd, sc, &sources, None, 500)?;
    drop(idx);
    if sessions.is_empty() {
        println!("No sessions found for {cwd}.\nTry `termem ls --all` to see everything.");
        return Ok(());
    }
    match tui::run(sessions, cwd)? {
        Some(s) => Err(resume::exec(&s)),
        None => Ok(()),
    }
}

fn cmd_hint(a: HintArgs) -> Result<()> {
    // Runs on every `cd`, so it reads the cached index without refreshing.
    // It prints nothing when there are no sessions (or no index yet).
    let idx = Index::open_default()?;
    let cwd = resolve_cwd(&a.cwd)?;
    let res = query::query(idx.conn(), &cwd, Scope::Subtree, &[], None, 1000)?;
    if res.is_empty() {
        return Ok(());
    }
    let (mut claude, mut codex, mut shell) = (0, 0, 0);
    for s in &res {
        match s.source {
            Source::Claude => claude += 1,
            Source::Codex => codex += 1,
            Source::Shell => shell += 1,
        }
    }
    let mut parts = Vec::new();
    if claude > 0 {
        parts.push(format!("{claude} claude"));
    }
    if codex > 0 {
        parts.push(format!("{codex} codex"));
    }
    if shell > 0 {
        parts.push(format!("{shell} shell"));
    }
    let noun = if res.len() == 1 { "session" } else { "sessions" };
    println!(
        "termem: {} {} here ({}). run 'termem' to resume.",
        res.len(),
        noun,
        parts.join(", ")
    );
    Ok(())
}

fn cmd_init(a: InitArgs) -> Result<()> {
    match shellhook::snippet(&a.shell) {
        Some(s) => {
            print!("{s}");
            Ok(())
        }
        None => {
            eprintln!("Unknown shell '{}'. Supported: zsh, bash", a.shell);
            std::process::exit(2);
        }
    }
}

// ---- helpers ----

fn scope_of(s: &ScopeArgs) -> Scope {
    if s.all {
        Scope::All
    } else if s.here {
        Scope::Here
    } else {
        Scope::Subtree
    }
}

fn sources_of(opt: &Option<String>) -> Vec<Source> {
    match opt {
        Some(s) => s
            .split(',')
            .filter_map(|x| Source::from_str(x.trim()))
            .collect(),
        None => Vec::new(),
    }
}

fn resolve_cwd(opt: &Option<PathBuf>) -> Result<String> {
    let p = match opt {
        Some(p) => p.clone(),
        None => std::env::current_dir()?,
    };
    let abs = std::fs::canonicalize(&p).unwrap_or(p);
    Ok(abs.to_string_lossy().to_string())
}

fn rel_path(child: &str, base: &str) -> String {
    if child == base {
        return ".".to_string();
    }
    let prefix = format!("{base}/");
    if let Some(rest) = child.strip_prefix(&prefix) {
        format!("./{rest}")
    } else {
        child.to_string()
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

fn print_table(sessions: &[Session], cwd: &str, scope: Scope) {
    if sessions.is_empty() {
        let hint = match scope {
            Scope::All => "No sessions indexed yet. Run `termem index`.",
            _ => "Try `--all`, a parent directory, or `termem index`.",
        };
        println!("No sessions found for {cwd}\n{hint}");
        return;
    }
    println!("{} session(s) for {}\n", sessions.len(), cwd);
    for s in sessions {
        println!(
            "  {:<6}  {:>4} ago  {:>4} msg   {}",
            s.source.as_str(),
            rel_time(s.updated_at),
            s.msg_count,
            s.title
        );
        let loc = rel_path(&s.cwd, cwd);
        let model = s.model.as_deref().unwrap_or("");
        if model.is_empty() {
            println!("          {}   id:{}", loc, short_id(&s.id));
        } else {
            println!("          {}   id:{}   [{}]", loc, short_id(&s.id), model);
        }
    }
    println!("\nResume:  termem resume <id|text>    Pick interactively:  termem");
}
