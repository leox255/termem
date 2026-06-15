#!/usr/bin/env node
"use strict";

// Thin launcher: find the prebuilt binary shipped in the matching
// per-platform optional dependency and exec it with the same args.
const { spawnSync } = require("node:child_process");

const pkg = `@termem/${process.platform}-${process.arch}`;

let binary;
try {
  binary = require.resolve(`${pkg}/termem`);
} catch {
  console.error(
    `termem: no prebuilt binary for ${process.platform}-${process.arch}.\n` +
      `Install another way: cargo install termem  (https://github.com/leox255/termem)`
  );
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}
process.exit(result.status === null ? 1 : result.status);
