---
name: termem
description: >
  Shared terminal memory across sessions and across coding agents. Use this skill
  whenever the user references prior work in this directory — "what was I doing here",
  "continue where we left off", "what did we decide about X", "did I already solve this",
  "pick up the migration from last week" — or whenever you start work in a directory and
  would benefit from knowing what happened in it before. Also use it before re-explaining
  context the user clearly assumes you already have, and whenever the user mentions a
  past session, a previous fix, or work done in another tool (Codex, Gemini, opencode,
  pi.dev). termem reads the session files those tools already write, so it can recover
  context even from sessions you did not create. Reach for it proactively — losing prior
  context and forcing the user to repeat themselves is the failure mode this prevents.
compatibility: Requires the `termem` binary (cargo install) and the termem MCP server
  registered for this agent (`claude mcp add termem -- termem mcp`).
---

# termem — shared terminal memory

termem is the memory layer underneath your terminal. It has already indexed every Claude
Code, Codex, and shell session that ran in this machine's directories, plus sessions from
other agents (Gemini CLI, opencode, pi.dev). You reach it through the termem MCP tools:
`recall`, `search`, `get_session`, `save_summary`, `stats`.

You are the intelligence; termem is the index and the store. **termem never calls a model
and never sends anything anywhere — you do the reasoning, and you are the only path by
which recalled text reaches a model.** Act accordingly (see Scope & safety).

## When you enter a directory

If the user's request implies prior context in this directory — or you're simply starting
work somewhere with history — call `recall` first:

```
recall(dir=<cwd>)                    # general "what happened here"
recall(dir=<cwd>, query="auth flow") # focused on a topic
```

You get back distilled primers for the recent relevant sessions. Use them to orient
silently — don't narrate "I called recall"; just continue as someone who remembers. If a
primer flags unfinished work, surface that to the user ("last time the migration script
was written but never run — want to pick that up?").

## When a session has no summary yet

`recall` marks each session `cached`, `needs_summary`, or `stale`. For `needs_summary` or
`stale`, build the memory so the next agent benefits:

1. `get_session(id=...)` — read the real transcript (paginate with the cursor).
2. Distil it into a tight primer: what was being worked on, what was decided, which files
   and commands mattered, and **what was left unfinished**.
3. `save_summary(id=..., summary=..., unfinished=...)` — store it.

That primer is now readable by *any* agent next time — that is the whole point of the
shared layer. Keep summaries short and factual; they are context, not prose.

## When the user asks a specific question about the past

"How did I configure nginx last month?" / "Which session fixed the CORS bug?"

1. `search(query=...)` to find candidate sessions (use `source` / `scope` to narrow).
2. `get_session` on the best match to read the actual detail.
3. Answer from what you read — cite the session id/date so the user can resume it.

## Scope & safety (do not skip)

- **Default to the current directory tree.** `recall` and `search` default to
  `scope: "tree"` (this directory and its subfolders). Only use `scope: "all"` after the
  user explicitly asks to look across all their projects — it can surface another client's
  or project's code into this conversation.
- **You are the egress point.** Recalled summaries and transcripts may end up in your
  model context. When the material you're about to use comes from a *different directory*
  than the current one, say so before using it.
- **Never invent a summary.** If a session is `needs_summary`, read it via `get_session`
  first. Don't guess what a past session contained.
- **`save_summary` is the only write.** Never attempt to modify the underlying session
  files — termem treats them as read-only and so must you.

## Resuming, not just recalling

When the user wants to actually re-open a past session rather than just hear about it, the
`termem` CLI handles that directly:

```
termem resume <id>           # reopen in the right tool and directory
termem resume <id> --print   # print the command instead of running it
```

Prefer a summarised handoff (recall + a fresh session primed with the primer) over
replaying a giant old transcript into context — it's cheaper and cleaner. Offer raw resume
when the user specifically wants the original thread back.
