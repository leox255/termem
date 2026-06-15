# termem

**Cross-agent memory and session management for your terminal.**

Coding agents forget everything between sessions, and none of them can see what the others did. termem is the shared memory layer underneath them all. It indexes every Claude Code, Codex, Gemini, opencode, and shell session by the directory it ran in, so you can:

- **Recall** prior work in a directory through the MCP server. An agent reads what happened there before, even sessions a different agent created, and picks up where you left off.
- **Resume** the exact past session in the right tool and directory.
- **Search** across everything by message content, not just titles.

termem never calls a model and never makes a network request. Your agents do the reasoning; termem does the retrieval and storage.

Run `termem` in a project folder to get a list of every session that started there or in a subfolder, with its title, age, and message count. Pick one and it reopens in the right tool and directory.

## Install

With a Rust toolchain (https://rustup.rs):

```
cargo install termem
```

Or with Node, no Rust required:

```
npx @termem/cli              # run it directly
npm install -g @termem/cli   # or install the `termem` command
```

Or download a prebuilt macOS / Linux binary from the [releases page](https://github.com/leox255/termem/releases), extract it, and put `termem` on your `PATH`.

From source:

```
cargo install --git https://github.com/leox255/termem
# or from a local clone:
git clone https://github.com/leox255/termem && cd termem && cargo install --path .
```

`cargo install` puts the `termem` binary in `~/.cargo/bin` (make sure that is on your `PATH`).

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

The MCP server is the `termem mcp` command, so there is nothing extra to download: registering just points your agent at the binary already on your `PATH`. The skill is a `SKILL.md` that termem generates.

Register it with Claude Code (user scope, so it works in every project):

```
claude mcp add termem --scope user -- termem mcp
mkdir -p ~/.claude/skills/termem && termem init claude > ~/.claude/skills/termem/SKILL.md
```

No binary on your `PATH`? Use `npx -y @termem/cli mcp` as the command instead of `termem mcp`.

Or get the skill and MCP server together as a plugin (still needs the `termem` binary on your `PATH`):

```
/plugin marketplace add leox255/termem
/plugin install termem@termem
```

Where they load from:

- MCP: `~/.claude.json` (user scope), or a project `.mcp.json` you commit to share with a team.
- Skill: `~/.claude/skills/termem/SKILL.md` (user-global), or `<repo>/.claude/skills/termem/SKILL.md` (per project).

Restart the agent (or start a new session) to pick them up.

For other agents, `termem init <agent>` prints the wrapper file and the one-line MCP registration for that tool:

```
termem init codex      # AGENTS.md  + ~/.codex/config.toml entry
termem init gemini     # GEMINI.md  + ~/.gemini/settings.json entry
termem init opencode   # AGENTS.md  + opencode.json entry
```

Every wrapper shares one body and one safety contract, so the workflow never drifts between agents.

The tools: `recall` (orient when you enter a directory), `search` (find a past session by message content or metadata), `get_session` (read a transcript, paginated), `save_summary` (store a primer for the next agent), and `stats`. `recall` and `search` default to the current directory tree; widening to the whole machine is explicit.

## How it works

termem reads the data the tools already write:

```
Claude Code   ~/.claude/projects/<dir>/<id>.jsonl
Codex         ~/.codex/sessions/.../rollout-*.jsonl
opencode      ~/.local/share/opencode/opencode.db          (session table)
Gemini        ~/.gemini/tmp/<project>/chats/session-*.jsonl
Shell         ~/.termem/shell/*.log                         (written by the shell hook)
```

Each session records the directory it ran in, so termem groups by directory. A shell session that moves between directories is listed under each one. Titles come from the title the tool already stored, or the first prompt if there is none. termem does not call any model and does not send your data anywhere.

Claude Code, Codex, opencode, and Gemini all resume the exact session (Gemini via `gemini --session-file`). Shell entries just take you back to the directory.

Sessions are cached in SQLite, keyed on each file's modification time and size, so only changed files are re-read. A full scan of a few hundred sessions takes well under a second and later scans are near instant.

## License

MIT
