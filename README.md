# termem

**Cross-agent memory and session management for your terminal.**

Coding agents forget everything between sessions, and none of them can see what the others did. termem is the shared memory layer underneath them all. It indexes every Claude Code, Codex, Gemini, opencode, and shell session by the directory it ran in, so you can:

- **Recall** prior work in a directory through the MCP server. An agent reads what happened there before, even sessions a different agent created, and picks up where you left off.
- **Resume** the exact past session in the right tool and directory.
- **Search** across everything by message content, not just titles.

termem never calls a model and never makes a network request. Your agents do the reasoning; termem does the retrieval and storage.

Run `termem` in a project folder to get a list of every session that started there or in a subfolder, with its title, age, and message count. Pick one and it reopens in the right tool and directory.

## Install

You need a Rust toolchain. Get one at https://rustup.rs.

Install from GitHub:

```
cargo install --git https://github.com/leox255/termem
```

Or from a local clone:

```
git clone https://github.com/leox255/termem
cd termem
cargo install --path .
```

Either way you get a `termem` binary in `~/.cargo/bin` (make sure that is on your `PATH`).

## Use

```
termem                       open the picker for the current directory and subfolders
termem --here                only sessions started exactly here
termem --all                 every session, any directory
termem ls                    print a table instead of opening the picker
termem ls --json             machine readable output
termem ls --source codex     filter by tool: claude, codex, opencode, gemini, shell
termem ls -s "query"         search message content, title, prompt, and path
termem resume <id|text>      resume the best match
termem resume <id> --print   print the command instead of running it
termem index                 rebuild the index now
```

In the picker: type to filter, arrow keys to move, Enter to resume, Esc to quit.

## Shell integration (optional)

This tracks plain shell history per directory, adds `tm` and `tmr` helpers, and prints a session count when you `cd` into a folder.

```
# ~/.zshrc
eval "$(termem init zsh)"

# ~/.bashrc
eval "$(termem init bash)"
```

`tm` opens the picker. `tmr <query>` resumes the best match without the picker. Set `TERMEM_NO_HINT=1` to turn off the message on `cd`.

## Shared memory (MCP)

termem also runs as an MCP server, so a coding agent can recall what happened in a directory before, even work done by a different agent. termem stores agent-written summaries in its own sidecar (never in the source files) and serves them to whatever agent asks next. It never calls a model and never makes a network request: the agent does the reasoning, termem does retrieval and storage.

Register it with Claude Code:

```
claude mcp add termem -- termem mcp
termem init claude > ~/.claude/skills/termem/SKILL.md
```

For other agents, `termem init <agent>` prints the right wrapper (claude, codex, opencode, gemini, pi). Every wrapper shares one body and one safety contract, so the workflow never drifts between agents.

The tools: `recall` (orient when you enter a directory), `search` (find a past session by message content or metadata), `get_session` (read a transcript, paginated), `save_summary` (store a primer for the next agent), and `stats`. `recall` and `search` default to the current directory tree; widening to the whole machine is explicit.

## How it works

termem reads the data the tools already write:

```
Claude Code   ~/.claude/projects/<dir>/<id>.jsonl
Codex         ~/.codex/sessions/.../rollout-*.jsonl
opencode      ~/.local/share/opencode/opencode.db   (session table)
Gemini        ~/.gemini/tmp/<project>/logs.json
Shell         ~/.termem/shell/*.log                  (written by the shell hook)
```

Each session records the directory it ran in, so termem groups by directory. A shell session that moves between directories is listed under each one. Titles come from the title the tool already stored, or the first prompt if there is none. termem does not call any model and does not send your data anywhere.

Resume support differs by tool. Claude Code, Codex, and opencode resume the exact session by id. Gemini has no resume-by-id on its CLI, so termem shows your Gemini prompts per directory and reopens `gemini` in that directory. Shell entries just take you back to the directory.

Sessions are cached in SQLite, keyed on each file's modification time and size, so only changed files are re-read. A full scan of a few hundred sessions takes well under a second and later scans are near instant.

## License

MIT
