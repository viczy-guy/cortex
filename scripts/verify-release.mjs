// Checks the files of a release before it becomes a draft: the exact set of
// files, a `latest.json` that names this version and points each platform at
// a file that is there with the signature that is there, and checksums that
// match. The signatures themselves are verify-update-signatures.mjs's job.
//
// Usage: node scripts/verify-release.mjs <directory> [<owner/repo>]
// The version is the one in src-tauri/tauri.conf.json.

import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const [dir, repo = "viczy-guy/cortex"] = process.argv.slice(2);
if (!dir) {
  console.error("usage: verify-release.mjs <directory> [<owner/repo>]");
  process.exit(2);
}
const { version } = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"));

/** Updater platform → its package. Each has a `.sig` next to it. */
const platforms = {
  "darwin-aarch64": "Cortex_aarch64-apple-darwin.app.tar.gz",
  "darwin-x86_64": "Cortex_x86_64-apple-darwin.app.tar.gz",
  "linux-x86_64-appimage": `Cortex_${version}_amd64.AppImage`,
  "windows-x86_64-nsis": `Cortex_${version}_x64-setup.exe`,
  "windows-x86_64-msi": `Cortex_${version}_x64_en-US.msi`,
};
const packages = Object.values(platforms);
const expected = [
  ...packages,
  ...packages.map((name) => `${name}.sig`),
  `Cortex_${version}_aarch64.dmg`,
  `Cortex_${version}_x64.dmg`,
  `Cortex_${version}_amd64.deb`,
  `Cortex-${version}-1.x86_64.rpm`,
  "latest.json",
  "SHA256SUMS.txt",
];

const errors = [];
const present = readdirSync(dir);
for (const name of expected) if (!present.includes(name)) errors.push(`missing file: ${name}`);
for (const name of present) if (!expected.includes(name)) errors.push(`unexpected file: ${name}`);

const read = (name) => (present.includes(name) ? readFileSync(join(dir, name)) : null);

const manifest = (() => {
  try {
    return JSON.parse(read("latest.json") ?? "null");
  } catch {
    errors.push("latest.json is not JSON");
    return null;
  }
})();
if (manifest) {
  if (manifest.version !== version) errors.push(`latest.json: version ${manifest.version}, expected ${version}`);
  const listed = Object.keys(manifest.platforms ?? {});
  for (const name of listed) if (!(name in platforms)) errors.push(`latest.json: unexpected platform ${name}`);
  for (const [platform, name] of Object.entries(platforms)) {
    const entry = manifest.platforms?.[platform];
    if (!entry) {
      errors.push(`latest.json: missing platform ${platform}`);
      continue;
    }
    const url = `https://github.com/${repo}/releases/download/v${version}/${name}`;
    if (entry.url !== url) errors.push(`latest.json: ${platform} points to ${entry.url}, expected ${url}`);
    const sig = read(`${name}.sig`)?.toString("utf8").trim();
    if (sig && entry.signature !== sig) errors.push(`latest.json: ${platform} signature differs from ${name}.sig`);
  }
}

const sums = read("SHA256SUMS.txt")?.toString("utf8").trim().split("\n") ?? [];
const summed = new Map(sums.map((line) => [line.slice(66), line.slice(0, 64)]));
for (const name of present) {
  if (name === "SHA256SUMS.txt") continue;
  const hash = createHash("sha256").update(read(name)).digest("hex");
  if (summed.get(name) !== hash) errors.push(`SHA256SUMS.txt: no matching checksum for ${name}`);
}
for (const name of summed.keys()) if (!present.includes(name)) errors.push(`SHA256SUMS.txt: lists ${name}, which is not there`);

for (const error of errors) console.error(`::error::${error}`);
if (errors.length === 0) console.log(`ok  ${present.length} files, version ${version}, ${packages.length} update platforms`);
process.exit(errors.length === 0 ? 0 : 1);
