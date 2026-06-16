//! Turn a selected session into the command that resumes it.

use crate::model::{Session, Source};

/// The program + args that resume a session (run in the session's cwd).
///
/// A session started in a permissions-bypass mode is resumed in the same mode:
/// the agents do not restore it on their own, so termem re-passes the flag.
pub fn command_for(s: &Session) -> Option<(String, Vec<String>)> {
    match s.source {
        Source::Claude => {
            let mut args = vec!["--resume".into(), s.id.clone()];
            if s.bypass {
                args.push("--dangerously-skip-permissions".into());
            }
            Some(("claude".into(), args))
        }
        Source::Codex => {
            // `codex resume [OPTIONS] [SESSION_ID]`: the flag precedes the id.
            let mut args = vec!["resume".into()];
            if s.bypass {
                args.push("--dangerously-bypass-approvals-and-sandbox".into());
            }
            args.push(s.id.clone());
            Some(("codex".into(), args))
        }
        Source::Opencode => Some(("opencode".into(), vec!["--session".into(), s.id.clone()])),
        // Gemini loads a specific session from its chats file.
        Source::Gemini => Some((
            "gemini".into(),
            vec!["--session-file".into(), s.file_path.clone()],
        )),
        // A shell "session" can't be resumed; we just return to its directory.
        Source::Shell => None,
    }
}

/// A copy-pasteable shell line: `cd <cwd> && <resume cmd>`.
pub fn print_line(s: &Session) -> String {
    match command_for(s) {
        Some((cmd, args)) if args.is_empty() => format!("cd {} && {}", shell_quote(&s.cwd), cmd),
        Some((cmd, args)) => {
            let arg_str = args
                .iter()
                .map(|a| shell_quote(a))
                .collect::<Vec<_>>()
                .join(" ");
            format!("cd {} && {} {}", shell_quote(&s.cwd), cmd, arg_str)
        }
        None => format!("cd {}", shell_quote(&s.cwd)),
    }
}

/// Replace the current process with the resume command. Only returns on error.
#[cfg(unix)]
pub fn exec(s: &Session) -> anyhow::Error {
    use std::os::unix::process::CommandExt;
    match command_for(s) {
        Some((cmd, args)) => {
            let err = std::process::Command::new(&cmd)
                .args(&args)
                .current_dir(&s.cwd)
                .exec();
            anyhow::anyhow!("failed to exec {cmd}: {err}")
        }
        None => anyhow::anyhow!("shell sessions can't be resumed; cd to {} instead", s.cwd),
    }
}

#[cfg(not(unix))]
pub fn exec(s: &Session) -> anyhow::Error {
    match command_for(s) {
        Some((cmd, args)) => {
            let status = std::process::Command::new(&cmd)
                .args(&args)
                .current_dir(&s.cwd)
                .status();
            match status {
                Ok(st) => std::process::exit(st.code().unwrap_or(0)),
                Err(e) => anyhow::anyhow!("failed to run {cmd}: {e}"),
            }
        }
        None => anyhow::anyhow!("shell sessions can't be resumed"),
    }
}

/// POSIX single-quote a string for safe shell embedding.
pub fn shell_quote(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:@".contains(c))
    {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Source;

    fn sess(source: Source, id: &str, cwd: &str) -> Session {
        Session {
            id: id.into(),
            source,
            file_path: "/f".into(),
            cwd: cwd.into(),
            title: "t".into(),
            first_prompt: String::new(),
            last_prompt: String::new(),
            model: None,
            git_branch: None,
            started_at: 0,
            updated_at: 0,
            msg_count: 0,
            bypass: false,
        }
    }

    #[test]
    fn claude_resume_line() {
        let s = sess(Source::Claude, "abc-123", "/my proj");
        assert_eq!(print_line(&s), "cd '/my proj' && claude --resume abc-123");
    }

    #[test]
    fn codex_resume_line() {
        let s = sess(Source::Codex, "uuid", "/p");
        assert_eq!(print_line(&s), "cd /p && codex resume uuid");
    }

    #[test]
    fn bypass_claude_re_passes_the_flag() {
        let mut s = sess(Source::Claude, "abc-123", "/p");
        s.bypass = true;
        assert_eq!(
            print_line(&s),
            "cd /p && claude --resume abc-123 --dangerously-skip-permissions"
        );
    }

    #[test]
    fn bypass_codex_re_passes_the_flag_before_the_id() {
        let mut s = sess(Source::Codex, "uuid", "/p");
        s.bypass = true;
        assert_eq!(
            print_line(&s),
            "cd /p && codex resume --dangerously-bypass-approvals-and-sandbox uuid"
        );
    }

    #[test]
    fn non_bypass_sessions_are_unchanged() {
        // Default sessions must never gain a dangerous flag.
        let s = sess(Source::Claude, "abc-123", "/p");
        assert!(!print_line(&s).contains("dangerously"));
        let s = sess(Source::Codex, "uuid", "/p");
        assert!(!print_line(&s).contains("dangerously"));
    }

    #[test]
    fn opencode_resume_line() {
        let s = sess(Source::Opencode, "ses_123", "/p");
        assert_eq!(print_line(&s), "cd /p && opencode --session ses_123");
    }

    #[test]
    fn gemini_loads_session_file() {
        let s = sess(Source::Gemini, "sid", "/p");
        // sess() sets file_path "/f".
        assert_eq!(print_line(&s), "cd /p && gemini --session-file /f");
    }

    #[test]
    fn shell_just_cds() {
        let s = sess(Source::Shell, "x", "/p");
        assert_eq!(print_line(&s), "cd /p");
    }

    #[test]
    fn quoting() {
        assert_eq!(shell_quote("plain"), "plain");
        assert_eq!(shell_quote("with space"), "'with space'");
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }
}
