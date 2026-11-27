// What's new picks the right versions (#180). Run by `npm run check`; Node
// strips the types of the file it imports.
import assert from "node:assert/strict";

import { MAX_SHOWN, newer, shown } from "../src/splash/versions.ts";

assert.ok(newer("0.10.0", "0.9.2"));
assert.ok(newer("1.0", "0.99.99"));
assert.ok(!newer("0.9.1", "0.9.1"));
assert.ok(!newer("0.9.1", "0.9.2"));
// Pre-releases come before their release; a build changes nothing.
assert.ok(newer("0.9.1", "0.9.1-beta.2"));
assert.ok(!newer("0.9.1-beta.2", "0.9.1"));
assert.ok(newer("0.9.2", "0.9.1+build.7"));
assert.ok(!newer("0.9.1+build.7", "0.9.1"));
assert.ok(!newer("0.9.1", "0.9.1+build.7"));

const releases = ["0.7.0", "0.9.2", "0.8.0", "0.9.0", "0.9.1", "0.6.0"].map((version) => ({ version }));
const versions = (list) => list.map((r) => r.version);
// Updated from 0.9.0 to 0.9.2: the skipped 0.9.1 too, the newest first.
assert.deepEqual(versions(shown(releases, "0.9.2", "0.9.0")), ["0.9.2", "0.9.1"]);
// From a patch with no entry of its own.
assert.deepEqual(versions(shown(releases, "0.9.2", "0.8.1")), ["0.9.2", "0.9.1", "0.9.0"]);
// Nothing newer than the running copy, and the last few only.
assert.deepEqual(versions(shown(releases, "0.9.1", null)), ["0.9.1", "0.9.0", "0.8.0", "0.7.0", "0.6.0"]);
assert.equal(shown(releases, null, null).length, MAX_SHOWN);
// A version with no entry after the one used last: nothing to show.
assert.deepEqual(shown(releases, "0.9.3", "0.9.2"), []);
