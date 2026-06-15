# Distributing termem

How to publish termem so people can install it and load the skill + MCP server.

## 1. Release binaries (GitHub Releases)

`.github/workflows/release.yml` builds macOS (arm64 + x86_64) and Linux
(x86_64 + arm64) binaries and attaches them to the release when you push a
version tag:

```
git tag v0.5.2
git push origin v0.5.2
```

The workflow uploads `termem-<target>.tar.gz` (+ a `.sha256`) to the release at
https://github.com/leox255/termem/releases. Users download, extract, and put
`termem` on their `PATH`.

Windows is not built yet: the binary compiles there, but the default data
locations are Unix paths and the shell hook is zsh/bash only.

### Homebrew (after the first release exists)

Create a tap repo `leox255/homebrew-tap` with a formula like:

```ruby
class Termem < Formula
  desc "Cross-agent terminal memory and session management"
  homepage "https://github.com/leox255/termem"
  version "0.5.2"
  on_macos do
    on_arm do
      url "https://github.com/leox255/termem/releases/download/v0.5.2/termem-aarch64-apple-darwin.tar.gz"
      sha256 "<from the .sha256 asset>"
    end
    on_intel do
      url "https://github.com/leox255/termem/releases/download/v0.5.2/termem-x86_64-apple-darwin.tar.gz"
      sha256 "<from the .sha256 asset>"
    end
  end
  def install
    bin.install "termem"
  end
end
```

Then `brew install leox255/tap/termem`.

## 2. npm (`npx @termem/cli`)

The release workflow also publishes npm packages so anyone with Node can run
`npx @termem/cli` (no Rust). It uses the esbuild-style per-platform packages
(see `npm/README.md` and `npm/publish.sh`), so there is no postinstall
download.

CI publishes with **OIDC trusted publishing** -- no `NPM_TOKEN` secret, and npm
records build provenance automatically. The `npm` job in `release.yml` already
has `id-token: write` and runs on npm 11.

npm only lets you attach a trusted publisher to a package that already exists,
so there is a one-time, token-free bootstrap (run locally):

1. Log in to npm: `npm login`.
2. Reserve the five package names (publishes a tiny `0.0.0` placeholder for
   each; the real release publishes a higher version over it):
   ```
   bash npm/reserve.sh
   ```
3. Add a trusted publisher for EACH package. For every one of `@termem/cli`,
   `@termem/darwin-arm64`, `@termem/darwin-x64`, `@termem/linux-x64`,
   `@termem/linux-arm64`, on npmjs.com open the package -> Settings ->
   Trusted Publisher -> GitHub Actions and enter: organization `leox255`,
   repository `termem`, workflow filename `release.yml`, environment blank.

   Or script it with the CLI (needs npm >= 11.17 for `--allow-publish`; older
   npm omits the now-required action field and the registry rejects it with an
   opaque 400):
   ```
   for p in @termem/cli @termem/darwin-arm64 @termem/darwin-x64 @termem/linux-x64 @termem/linux-arm64; do
     npm trust github "$p" --repo leox255/termem --file release.yml --allow-publish
   done
   ```
4. Cut a release (push a `v*` tag). The `npm` job publishes all five packages
   keylessly via OIDC.

The npm package is `@termem/cli`, published under the `termem` org. It installs
a plain `termem` command, so `npx @termem/cli`, `npm install -g @termem/cli`
(gives `termem`), and MCP configs like
`{ "command": "npx", "args": ["-y", "@termem/cli", "mcp"] }` work on
macOS/Linux.

## 3. Claude Code plugin

This repo is itself a plugin marketplace:

- `.claude-plugin/marketplace.json` lists the `termem` plugin.
- `plugin/` is the plugin: `.claude-plugin/plugin.json` (registers the MCP
  server) and `skills/termem/SKILL.md` (the skill, the single canonical copy the
  binary also embeds).

Users install both the skill and the MCP server in one step:

```
/plugin marketplace add leox255/termem
/plugin install termem@termem
```

The plugin still needs the `termem` binary on `PATH` (the MCP server is
`termem mcp`).

## 4. MCP registries

`server.json` is the manifest for the official registry
(registry.modelcontextprotocol.io).

Prerequisite: the registry installs the server from a package registry. The
`server.json` here declares `registryType: cargo`, so it requires the crate to
be published first:

```
cargo publish            # publishes termem to crates.io
mcp-publisher login github
mcp-publisher publish     # reads ./server.json
```

If you do not want to publish to crates.io, the alternatives are an npm wrapper
package (that downloads the release binary) or an MCPB bundle, then change
`registryType` accordingly.

Once on the official registry, downstream directories (Smithery, mcp.so,
PulseMCP, the GitHub MCP registry) ingest it automatically, usually within a
week. They can also be submitted manually at their own sites.
