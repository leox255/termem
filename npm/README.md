# npm packaging for termem

`npx termem` and `npm install -g termem` work via the esbuild-style
"optional per-platform packages" pattern, so there is no Rust toolchain and no
postinstall download script.

- `termem` (main): a tiny launcher (`termem/bin/termem.js`) plus
  `optionalDependencies` on the per-platform packages.
- `termem-<os>-<cpu>` (one per platform): just the prebuilt binary.

`npm` installs only the package matching the user's platform; the launcher
execs that binary with the given args (so `npx termem mcp` runs the MCP server).

These are published by the `npm` job in `.github/workflows/release.yml` when a
`v*` tag is pushed. `npm/publish.sh` generates each package from the release
binaries in `./dist` and runs `npm publish`. Set an `NPM_TOKEN` repo secret
(an npm automation token) so CI can publish.

Covered today: macOS (arm64, x64) and Linux (x64, arm64). Windows is not built
yet, so `npx termem` reports an unsupported platform there.
