# Changelog

All notable changes to termem are documented here.

## [0.5.1] - 2026-06-15

### Changed
- Restyled the interactive picker: a `termem` wordmark (teal/violet), rounded
  panels, a cohesive palette, colored per-source badges, and a cleaner key bar.

## [0.5.0] - 2026-06-15

### Added
- Content search: an FTS5 index over message bodies. `search` (the MCP tool and
  CLI `ls -s`) now finds sessions by what was discussed, not just titles and
  prompts. The index is maintained incrementally alongside the session cache,
  with a per-session body cap so a giant transcript stays bounded.

### Fixed
- The `--source` filter was ignored when 3 or 4 tools were selected (a stale
  bound from when there were only three sources); it now applies for any subset.

## [0.4.0] - 2026-06-15

### Added
- `termem mcp`: an MCP server over stdio exposing `recall`, `search`,
  `get_session`, `save_summary`, and `stats`, so agents recall prior work and
  build shared memory across sessions and across tools. No model calls, no
  network.
- Durable agent-authored summaries in termem's own store (never the source
  files), with cached / needs_summary / stale freshness tracking.
- `termem init <agent>` emits the skill / AGENTS.md / GEMINI.md wrapper from one
  canonical `SKILL.md`, so the workflow and safety contract never drift between
  agents.
- Environment overrides `TERMEM_DB` and `TERMEM_<source>_DIR` for custom or
  synced locations.

## [0.3.0] - 2026-06-15

### Added
- opencode support: reads the `session` table from
  `~/.local/share/opencode/opencode.db` (directory, title, timestamps) and
  resumes by id with `opencode --session <id>`.
- Gemini support: reads `~/.gemini/tmp/<project>/logs.json` and attributes
  sessions to directories via `projects.json`. Gemini has no resume-by-id, so
  these are browse + reopen `gemini` in the directory.
- Scan roots for both tools are configurable via `ScanRoots`.

## [0.2.0] - 2026-06-15

### Added
- Shell sessions are indexed per directory, so a session that moves between
  directories shows up under each one.
- `termem hint` prints a one-line session count, and the zsh/bash hooks call it
  when you `cd` into a directory. Opt out with `TERMEM_NO_HINT=1`.

### Changed
- `resume` resolves an exact session id first, then an id prefix, then fuzzy text.
- README rewritten.

### Fixed
- Guard shell-log timestamp parsing against integer overflow.
- Escape `%` and `_` in search so they match literally.
- The cd hint opens the index read-only (never rebuilds or migrates it) and
  counts with an aggregate query instead of capping at 1000 rows.
- A changed session file that briefly fails to parse keeps its previous index
  rows instead of being dropped.

## [0.1.0] - 2026-06-15

### Added
- Index Claude Code, Codex, and shell sessions and resume them by directory.
- Interactive ratatui picker, `ls` / `resume` / `index` subcommands, and
  zsh/bash shell integration.
- Incremental SQLite index keyed on file mtime and size.
