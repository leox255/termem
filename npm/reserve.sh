#!/usr/bin/env bash
# One-time bootstrap for npm trusted publishing.
#
# npm only lets you attach a trusted publisher to a package that already
# exists. This reserves the five package names by publishing a tiny 0.0.0
# placeholder for each; the real release (via OIDC in release.yml) publishes a
# higher version over them.
#
# Run once, locally, after `npm login`. No token needed.
#   npm login
#   bash npm/reserve.sh
# Then configure a trusted publisher for each package (see DISTRIBUTION.md) and
# cut a release.
set -euo pipefail

names=(
  termem
  termem-darwin-arm64
  termem-darwin-x64
  termem-linux-x64
  termem-linux-arm64
)

for name in "${names[@]}"; do
  if npm view "$name" version >/dev/null 2>&1; then
    echo "exists, skipping: $name"
    continue
  fi
  d="$(mktemp -d)"
  cat >"$d/package.json" <<EOF
{
  "name": "$name",
  "version": "0.0.0",
  "description": "Reserved for termem; real builds publish over this via trusted publishing.",
  "license": "MIT",
  "repository": { "type": "git", "url": "git+https://github.com/leox255/termem.git" }
}
EOF
  echo "reserving $name@0.0.0"
  (cd "$d" && npm publish --access public)
done

echo
echo "done. Next: add a trusted publisher for each package (DISTRIBUTION.md), then push a v* tag."
