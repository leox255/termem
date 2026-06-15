use serde::Serialize;

/// Where a session came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Claude,
    Codex,
    Shell,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Claude => "claude",
            Source::Codex => "codex",
            Source::Shell => "shell",
        }
    }

    pub fn from_str(s: &str) -> Option<Source> {
        match s {
            "claude" => Some(Source::Claude),
            "codex" => Some(Source::Codex),
            "shell" => Some(Source::Shell),
            _ => None,
        }
    }
}

/// One resumable session, normalized across tools.
#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub id: String,
    pub source: Source,
    pub file_path: String,
    pub cwd: String,
    pub title: String,
    pub first_prompt: String,
    pub last_prompt: String,
    pub model: Option<String>,
    pub git_branch: Option<String>,
    /// Unix epoch milliseconds.
    pub started_at: i64,
    pub updated_at: i64,
    pub msg_count: i64,
}

/// Human-friendly "time ago" from an epoch-ms timestamp.
pub fn rel_time(ms: i64) -> String {
    let now = chrono::Utc::now().timestamp_millis();
    let secs = (now - ms).max(0) / 1000;
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86_400)
    }
}

/// Collapse a free-text prompt into a one-line title.
pub fn truncate_title(s: &str) -> String {
    let line = s.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    let max = 72;
    if line.chars().count() <= max {
        line.to_string()
    } else {
        let truncated: String = line.chars().take(max).collect();
        format!("{}…", truncated.trim_end())
    }
}
