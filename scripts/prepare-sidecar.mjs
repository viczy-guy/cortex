// Builds the relay (cortex-hook) for the bundle's target and puts it where
// Tauri's `externalBin` looks for it: src-tauri/binaries/cortex-hook-<triple>.
//
// Runs as the `beforeBuildCommand` of the bundle config
// (src-tauri/tauri.bundle.conf.json). Plain `cargo build/test/clippy` and
// `tauri dev` never need it: `externalBin` is only declared for bundling.

import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

function hostTriple() {
  const info = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
  const host = /^host: (.+)$/m.exec(info);
  if (!host) throw new Error("cannot read the host triple from `rustc -vV`");
  return host[1].trim();
}

// Set by the Tauri CLI for build hooks; absent when run by hand.
const triple = process.env.TAURI_ENV_TARGET_TRIPLE || hostTriple();
if (!/^[a-z0-9_.-]+$/.test(triple)) throw new Error(`unexpected target triple: ${triple}`);
const exe = triple.includes("windows") ? ".exe" : "";

// Always pass --target so the output path does not depend on how the app
// itself is being built.
execFileSync(
  "cargo",
  ["build", "--release", "--locked", "-p", "cortex-hook", "--target", triple],
  { cwd: root, stdio: "inherit" },
);

const built = join(root, "target", triple, "release", `cortex-hook${exe}`);
const dest = join(root, "src-tauri", "binaries", `cortex-hook-${triple}${exe}`);
mkdirSync(dirname(dest), { recursive: true });
copyFileSync(built, dest);
console.log(`relay ready: ${dest}`);
