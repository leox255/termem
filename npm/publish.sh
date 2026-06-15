#!/usr/bin/env bash
# Publish the npm packages for a release: one per-platform package per binary
# (containing the prebuilt binary), then the main `termem` package whose
# optionalDependencies point at them. Binaries are read from ./dist as
# `termem-<rust-target>.tar.gz` (downloaded from the GitHub release).
#
# Usage: npm/publish.sh <version>   e.g. npm/publish.sh 0.5.2
set -euo pipefail

VERSION="${1:?usage: publish.sh <version>}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="${DIST:-$ROOT/dist}"

# npm refuses to publish over an existing version, so a tag re-push or a CI
# re-run would otherwise fail the job. Skip any package@version already on the
# registry: that makes re-runs safe no-ops and recovers a partial publish (only
# the missing packages get pushed).
already_published() {
  local v
  v="$(npm view "$1" version 2>/dev/null || true)"
  [ -n "$v" ]
}

# rust target : node platform : node arch : binary name
targets=(
  "aarch64-apple-darwin:darwin:arm64:termem"
  "x86_64-apple-darwin:darwin:x64:termem"
  "x86_64-unknown-linux-gnu:linux:x64:termem"
  "aarch64-unknown-linux-gnu:linux:arm64:termem"
  "x86_64-pc-windows-msvc:win32:x64:termem.exe"
)

deps_json=""
sep=""
for entry in "${targets[@]}"; do
  IFS=":" read -r target os cpu binname <<<"$entry"
  tarball="$DIST/termem-$target.tar.gz"
  if [ ! -f "$tarball" ]; then
    echo "skip $target (no $tarball)"
    continue
  fi
  pkg="@termem/$os-$cpu"
  work="$(mktemp -d)"
  tar -xzf "$tarball" -C "$work"
  chmod +x "$work/$binname"
  cat >"$work/package.json" <<EOF
{
  "name": "$pkg",
  "version": "$VERSION",
  "description": "termem prebuilt binary for $os-$cpu",
  "license": "MIT",
  "repository": { "type": "git", "url": "git+https://github.com/leox255/termem.git" },
  "os": ["$os"],
  "cpu": ["$cpu"],
  "files": ["$binname"]
}
EOF
  if already_published "$pkg@$VERSION"; then
    echo "skip $pkg@$VERSION (already on npm)"
  else
    echo "publishing $pkg@$VERSION"
    (cd "$work" && npm publish --access public)
  fi
  deps_json+="${sep}    \"$pkg\": \"$VERSION\""
  sep=$',\n'
done

if [ -z "$deps_json" ]; then
  echo "no platform packages published (no binaries in $DIST)" >&2
  exit 1
fi

# Main package: launcher + optionalDependencies on the platform packages.
main="$(mktemp -d)"
mkdir -p "$main/bin"
cp "$ROOT/npm/termem/bin/termem.js" "$main/bin/termem.js"
[ -f "$ROOT/README.md" ] && cp "$ROOT/README.md" "$main/README.md"
cat >"$main/package.json" <<EOF
{
  "name": "@termem/cli",
  "version": "$VERSION",
  "description": "Cross-agent terminal memory and session management: recall, search, and resume Claude Code, Codex, Gemini, opencode, and shell sessions by directory.",
  "license": "MIT",
  "repository": { "type": "git", "url": "git+https://github.com/leox255/termem.git" },
  "homepage": "https://github.com/leox255/termem",
  "bin": { "termem": "bin/termem.js" },
  "files": ["bin/termem.js"],
  "keywords": ["mcp", "memory", "sessions", "cli", "terminal"],
  "mcpName": "io.github.leox255/termem",
  "optionalDependencies": {
$deps_json
  }
}
EOF
if already_published "@termem/cli@$VERSION"; then
  echo "skip @termem/cli@$VERSION (already on npm)"
else
  echo "publishing @termem/cli@$VERSION"
  (cd "$main" && npm publish --access public)
fi
echo "done"
