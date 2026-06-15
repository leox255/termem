//! Shell integration snippets emitted by `termem init <shell>`.
//!
//! The hook records `<epoch>\t<cwd>\t<command>` per command into a per-session
//! log under `~/.termem/shell/`, making shell history directory-aware. It also
//! defines `tm` (open the picker) and `tmr` (resume by query).

pub fn snippet(shell: &str) -> Option<&'static str> {
    match shell {
        "zsh" => Some(ZSH),
        "bash" => Some(BASH),
        _ => None,
    }
}

const ZSH: &str = r#"# termem shell integration (zsh) — add to ~/.zshrc:  eval "$(termem init zsh)"
typeset -g TERMEM_SESSION="${TERMEM_SESSION:-${$}-${RANDOM}}"
typeset -g _TERMEM_LOG="$HOME/.termem/shell/${TERMEM_SESSION}.log"
[[ -d "$HOME/.termem/shell" ]] || mkdir -p "$HOME/.termem/shell"
_termem_preexec() {
  print -r -- "$(date +%s)	$PWD	$1" >> "$_TERMEM_LOG"
}
autoload -Uz add-zsh-hook 2>/dev/null
if (( $+functions[add-zsh-hook] )); then
  add-zsh-hook preexec _termem_preexec
else
  preexec_functions+=(_termem_preexec)
fi
# tm: open the picker for the current directory.  tmr <query>: resume best match.
tm()  { command termem tui "$@" }
tmr() { local c; c="$(command termem resume --print "$@")" && eval "$c" }
"#;

const BASH: &str = r#"# termem shell integration (bash) — add to ~/.bashrc:  eval "$(termem init bash)"
export TERMEM_SESSION="${TERMEM_SESSION:-$$-$RANDOM}"
export _TERMEM_LOG="$HOME/.termem/shell/${TERMEM_SESSION}.log"
[ -d "$HOME/.termem/shell" ] || mkdir -p "$HOME/.termem/shell"
_termem_log_cmd() {
  [ -n "$COMP_LINE" ] && return
  [ "$BASH_COMMAND" = "$PROMPT_COMMAND" ] && return
  printf '%s\t%s\t%s\n' "$(date +%s)" "$PWD" "$BASH_COMMAND" >> "$_TERMEM_LOG"
}
trap '_termem_log_cmd' DEBUG
tm()  { command termem tui "$@"; }
tmr() { local c; c="$(command termem resume --print "$@")" && eval "$c"; }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_known_shells() {
        assert!(snippet("zsh").unwrap().contains("_termem_preexec"));
        assert!(snippet("bash").unwrap().contains("trap '_termem_log_cmd' DEBUG"));
        assert!(snippet("fish").is_none());
    }
}
