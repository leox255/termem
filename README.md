# termem — terminal memory

`cd` into a folder, run `termem`, and see **every** AI/coding session that was
ever started there — Claude Code, Codex, and (opt-in) shell history — with a
title, age, and message count. Pick one and it resumes instantly.

No more `claude --resume` roulette or digging through `~/.codex/sessions`.

```
$ termem
┌ filter: _ ──────────────────────────────────────────────────┐
│ 12 session(s) · /Users/you/ai/apps/termem                     │
└───────────────────────────────────────────────────────────────┘
┌ sessions ───────────────────┐┌ preview ─────────────────────┐
│▶ ◆  2m  Build terminal memo…││ Build terminal memory system  │
│  ◇  1h  Refactor the importer││                               │
│  ◆  3h  Fix the WAL deadlock ││ source  claude                │
│  ❯  1d  cargo test loop      ││ updated 2m ago · 123 msgs     │
└──────────────────────────────┘│ model   claude-opus-4-8       │
 ↑↓ move  ⏎ resume  type filter └───────────────────────────────┘
```

## Why it's cheap and fast

- **Zero tokens.** termem never calls an LLM. Titles are read from data the
  tools already wrote: Claude Code's `ai-title` line, Codex's `thread_name`,
  or the first user prompt as a fallback.
- **Incremental index.** Sessions are cached in SQLite keyed on
  `(file mtime, size)`. The first scan of ~388 MB / 345 sessions takes <1s
  (parsed in parallel with rayon); every scan after that is ~10 ms because only
  changed files are re-read.
- **Single static binary**, written in Rust.

## How it works

| Source | Location | Title from | Resume |
|--------|----------|-----------|--------|
| Claude Code | `~/.claude/projects/<enc-cwd>/<uuid>.jsonl` | `ai-title` → first prompt | `claude --resume <id>` |
| Codex | `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` | `session_index.jsonl` `thread_name` → first prompt | `codex resume <id>` |
| Shell (opt-in) | `~/.termem/shell/<session>.log` | first command | `cd <dir>` |

Every session is attributed to the directory it was started in (Claude/Codex
record `cwd` in their transcripts). termem groups by that, so a project folder
shows exactly the sessions that happened there or in any subdirectory.

## Install

```sh
cargo install --path .
# or: cargo build --release && cp target/release/termem ~/.local/bin/
```

## Usage

```sh
termem                       # interactive picker for the current directory (+ subdirs)
termem --here                # only sessions started exactly here
termem --all                 # every session, anywhere
termem ls                    # non-interactive table
termem ls --json             # machine-readable
termem ls --source codex     # filter by tool (claude,codex,shell)
termem ls -s "wal deadlock"  # substring search across title/prompt/cwd
termem resume <id|text>      # resume best match (cd's + launches the tool)
termem resume <id> --print   # just print the `cd … && …` command
termem index                 # refresh the index now (also done automatically)
```

Inside the picker: type to fuzzy-filter, `↑/↓` (or `Ctrl-N/P`) to move,
`Enter` to resume, `Esc` to quit.

## Shell integration (optional)

Makes plain shell history directory-aware and adds `tm` / `tmr` helpers:

```sh
# ~/.zshrc
eval "$(termem init zsh)"
# ~/.bashrc
eval "$(termem init bash)"
```

- `tm` — open the picker for the current directory.
- `tmr <query>` — resume the best match without opening the picker.

The hook appends `epoch⇥cwd⇥command` to a per-session log under
`~/.termem/shell/`, so future shell sessions show up alongside Claude/Codex.

## Custom session locations

By default termem scans `~/.claude/projects`, `~/.codex/sessions`, and
`~/.termem/shell`. The scan roots are injectable in the library
(`ScanRoots`) for synced/non-standard layouts.

## License

MIT
