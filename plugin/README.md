# termem (Claude Code plugin)

This plugin bundles the termem skill and registers the termem MCP server
(`recall`, `search`, `get_session`, `save_summary`, `stats`).

It requires the `termem` binary on your `PATH`. Install it first:

```
cargo install --git https://github.com/leox255/termem
```

(or download a release binary from
https://github.com/leox255/termem/releases).

Then, in Claude Code:

```
/plugin marketplace add leox255/termem
/plugin install termem@termem
```

Restart Claude Code (or start a new session) to load the skill and MCP server.
