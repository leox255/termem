//! Shell integration snippets emitted by `termem init <shell>`.
//!
//! The hook records `<epoch>\t<cwd>\t<command>` per command into a per-session
//! log under `~/.termem/shell/`, making shell history directory-aware. It also
//! defines `tm` (open the picker) and `tmr` (resume by query), and prints a
//! session count when you `cd` into a directory (set `TERMEM_NO_HINT=1` to
//! turn that off).

pub fn snippet(shell: &str) -> Option<&'static str> {
    match shell {
        "zsh" => Some(ZSH),
        "bash" => Some(BASH),
        "powershell" | "pwsh" => Some(POWERSHELL),
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
_termem_chpwd() {
  [[ -n "$TERMEM_NO_HINT" ]] && return
  command termem hint 2>/dev/null
}

autoload -Uz add-zsh-hook 2>/dev/null
if (( $+functions[add-zsh-hook] )); then
  add-zsh-hook preexec _termem_preexec
  add-zsh-hook chpwd _termem_chpwd
else
  preexec_functions+=(_termem_preexec)
  chpwd_functions+=(_termem_chpwd)
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

_termem_chpwd() {
  [ -n "$TERMEM_NO_HINT" ] && return
  if [ "$PWD" != "$_TERMEM_LAST_PWD" ]; then
    _TERMEM_LAST_PWD="$PWD"
    command termem hint 2>/dev/null
  fi
}
case ";$PROMPT_COMMAND;" in
  *";_termem_chpwd;"*) ;;
  *) PROMPT_COMMAND="_termem_chpwd${PROMPT_COMMAND:+;$PROMPT_COMMAND}" ;;
esac

tm()  { command termem tui "$@"; }
tmr() { local c; c="$(command termem resume --print "$@")" && eval "$c"; }
"#;

const POWERSHELL: &str = r#"# termem shell integration (PowerShell) — add to $PROFILE:
#   Invoke-Expression (& termem init powershell | Out-String)
if (-not $env:TERMEM_SESSION) { $env:TERMEM_SESSION = "$PID-$(Get-Random)" }
$global:_TermemLog = Join-Path $HOME ".termem\shell\$($env:TERMEM_SESSION).log"
$global:_TermemShellDir = Split-Path -Parent $global:_TermemLog
if (-not (Test-Path $global:_TermemShellDir)) { New-Item -ItemType Directory -Force -Path $global:_TermemShellDir | Out-Null }

if (-not $global:_TermemInstalled) {
  $global:_TermemInstalled = $true
  $global:_TermemLastPwd = ""
  $global:_TermemLastId = 0
  $global:_TermemOrigPrompt = $function:prompt

  function global:prompt {
    # Record the latest command once (history Id is monotonic per session).
    $h = Get-History -Count 1 -ErrorAction SilentlyContinue
    if ($h -and $h.Id -gt $global:_TermemLastId) {
      $global:_TermemLastId = $h.Id
      $ts = [DateTimeOffset]::Now.ToUnixTimeSeconds()
      [System.IO.File]::AppendAllText($global:_TermemLog, "$ts`t$($PWD.Path)`t$($h.CommandLine)`n")
    }
    # Session-count hint on directory change (set TERMEM_NO_HINT=1 to disable).
    if (-not $env:TERMEM_NO_HINT -and $PWD.Path -ne $global:_TermemLastPwd) {
      $global:_TermemLastPwd = $PWD.Path
      termem hint 2>$null
    }
    if ($global:_TermemOrigPrompt) { & $global:_TermemOrigPrompt } else { "PS $($PWD.Path)> " }
  }
}

# tm: open the picker for the current directory.  tmr <query>: resume best match.
function global:tm  { termem tui @args }
function global:tmr { $c = termem resume --print @args; if ($c) { Invoke-Expression $c } }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_known_shells() {
        let zsh = snippet("zsh").unwrap();
        assert!(zsh.contains("_termem_preexec"));
        assert!(zsh.contains("add-zsh-hook chpwd _termem_chpwd"));
        let bash = snippet("bash").unwrap();
        assert!(bash.contains("trap '_termem_log_cmd' DEBUG"));
        assert!(bash.contains("_termem_chpwd"));
        let ps = snippet("powershell").unwrap();
        assert!(ps.contains("function global:prompt"));
        assert!(ps.contains("termem hint"));
        assert_eq!(snippet("pwsh"), Some(ps));
        assert!(snippet("fish").is_none());
    }
}
