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

# rust target : node platform : node arch
targets=(
  "aarch64-apple-darwin:darwin:arm64"
  "x86_64-apple-darwin:darwin:x64"
  "x86_64-unknown-linux-gnu:linux:x64"
  "aarch64-unknown-linux-gnu:linux:arm64"
)

deps_json=""
sep=""
for entry in "${targets[@]}"; do
  IFS=":" read -r target os cpu <<<"$entry"
  tarball="$DIST/termem-$target.tar.gz"
  if [ ! -f "$tarball" ]; then
    echo "skip $target (no $tarball)"
    continue
  fi
  pkg="termem-$os-$cpu"
  work="$(mktemp -d)"
  tar -xzf "$tarball" -C "$work"
  chmod +x "$work/termem"
  cat >"$work/package.json" <<EOF
{
  "name": "$pkg",
  "version": "$VERSION",
  "description": "termem prebuilt binary for $os-$cpu",
  "license": "MIT",
  "repository": "https://github.com/leox255/termem",
  "os": ["$os"],
  "cpu": ["$cpu"],
  "files": ["termem"]
}
EOF
  echo "publishing $pkg@$VERSION"
  (cd "$work" && npm publish --access public)
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
  "name": "termem",
  "version": "$VERSION",
  "description": "Cross-agent terminal memory and session management: recall, search, and resume Claude Code, Codex, Gemini, opencode, and shell sessions by directory.",
  "license": "MIT",
  "repository": "https://github.com/leox255/termem",
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
echo "publishing termem@$VERSION"
(cd "$main" && npm publish --access public)
echo "done"
