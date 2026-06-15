//! The skill/wrapper text shipped to each agent, all derived from one canonical
//! `SKILL.md` so the workflow and safety contract never drift between agents.
//! `termem init <agent>` prints the right wrapper for each.

const CLAUDE_SKILL: &str = include_str!("../plugin/skills/termem/SKILL.md");

/// The workflow body (everything after the YAML frontmatter).
fn body() -> &'static str {
    let s = CLAUDE_SKILL;
    if let Some(rest) = s.strip_prefix("---\n") {
        if let Some(idx) = rest.find("\n---\n") {
            return rest[idx + 5..].trim_start();
        }
    }
    s
}

/// One-line MCP registration hint for an agent.
pub fn registration(agent: &str) -> &'static str {
    match agent {
        "claude" => "claude mcp add termem -- termem mcp",
        "codex" => "~/.codex/config.toml: [mcp_servers.termem] command=\"termem\" args=[\"mcp\"]",
        "opencode" => "opencode.json mcp: { \"termem\": { \"command\": [\"termem\", \"mcp\"] } }",
        "gemini" => "~/.gemini/settings.json mcpServers.termem: { \"command\": \"termem\", \"args\": [\"mcp\"] }",
        _ => "register an MCP server with command: termem mcp",
    }
}

/// The wrapper file contents for an agent, or `None` if the agent is unknown.
pub fn for_agent(agent: &str) -> Option<String> {
    match agent {
        "claude" => Some(CLAUDE_SKILL.to_string()),
        "codex" | "opencode" | "pi" | "gemini" => {
            let filename = if agent == "gemini" {
                "GEMINI.md"
            } else {
                "AGENTS.md"
            };
            Some(format!(
                "# termem — shared terminal memory\n\n\
                 Setup (run once): {reg}\n\n\
                 This file ({filename}) gives this agent the termem memory workflow. The \
                 body below is identical across agents.\n\n{body}\n",
                reg = registration(agent),
                body = body(),
            ))
        }
        _ => None,
    }
}

pub fn known_agents() -> &'static [&'static str] {
    &["claude", "codex", "opencode", "gemini", "pi"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_wrapper_is_the_full_skill() {
        let s = for_agent("claude").unwrap();
        assert!(s.starts_with("---\nname: termem"));
        assert!(s.contains("# termem — shared terminal memory"));
    }

    #[test]
    fn agents_share_the_body_without_frontmatter() {
        let codex = for_agent("codex").unwrap();
        assert!(!codex.starts_with("---")); // no YAML frontmatter
        assert!(codex.contains("Setup (run once): ~/.codex/config.toml"));
        assert!(codex.contains("## Scope & safety")); // same body
        assert!(for_agent("gemini").unwrap().contains("GEMINI.md"));
        assert!(for_agent("unknown").is_none());
    }
}
