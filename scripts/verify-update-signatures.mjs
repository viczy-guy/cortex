// Checks each update package against its `.sig` with the updater public key
// in src-tauri/tauri.conf.json — the key compiled into the app. A release whose
// signing secret does not match that key would ship updates every installed
// app rejects, so the release workflow runs this before anything is uploaded.
//
// Usage: node scripts/verify-update-signatures.mjs <package>...
// Formats (minisign, as written by `tauri signer`): the config key and each
// `.sig` are base64 of the minisign text files.

import { createHash, createPublicKey, verify } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

/** The lines of a base64-wrapped minisign text file. */
function minisignLines(base64) {
  return Buffer.from(base64.trim(), "base64").toString("utf8").trim().split("\n");
}

function publicKey() {
  const conf = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"));
  const bytes = Buffer.from(minisignLines(conf.plugins.updater.pubkey).at(-1), "base64");
  // "Ed" | key id (8) | Ed25519 key (32)
  if (bytes.length !== 42 || bytes.subarray(0, 2).toString() !== "Ed") {
    throw new Error("the updater pubkey in tauri.conf.json is not a minisign public key");
  }
  const key = createPublicKey({
    key: { kty: "OKP", crv: "Ed25519", x: bytes.subarray(10).toString("base64url") },
    format: "jwk",
  });
  return { id: bytes.subarray(2, 10), key };
}

function check(file, { id, key }) {
  const [, sigLine, trustedLine, globalLine] = minisignLines(readFileSync(`${file}.sig`, "utf8"));
  // "ED" (BLAKE2b-512 prehashed) or "Ed" | key id (8) | signature (64)
  const sig = Buffer.from(sigLine ?? "", "base64");
  if (sig.length !== 74) throw new Error("malformed signature");
  if (!sig.subarray(2, 10).equals(id)) throw new Error("signed with another key");
  const data = readFileSync(file);
  const message = sig.subarray(0, 2).toString() === "ED" ? createHash("blake2b512").update(data).digest() : data;
  if (!verify(null, message, key, sig.subarray(10))) throw new Error("the signature does not match the file");
  const trusted = (trustedLine ?? "").replace(/^trusted comment: /, "");
  const global = Buffer.from(globalLine ?? "", "base64");
  if (!verify(null, Buffer.concat([sig.subarray(10), Buffer.from(trusted)]), key, global)) {
    throw new Error("the trusted comment is not signed");
  }
  return trusted;
}

const files = process.argv.slice(2);
if (files.length === 0) {
  console.error("usage: verify-update-signatures.mjs <package>...");
  process.exit(2);
}
let failed = false;
const key = (() => {
  try {
    return publicKey();
  } catch (error) {
    console.error(`::error::${error.message}`);
    process.exit(1);
  }
})();
for (const file of files) {
  try {
    console.log(`ok  ${file}  (${check(file, key)})`);
  } catch (error) {
    console.error(`::error::${file}: ${error.message}`);
    failed = true;
  }
}
process.exit(failed ? 1 : 0);
