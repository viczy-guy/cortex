/**
 * Which versions the "What's new" screen shows (#180). No imports: the check
 * in `scripts/check-news.mjs` runs it under Node.
 */

/** The screen keeps to the last few versions, as the app carries their images. */
export const MAX_SHOWN = 5;

/** "1.0.0" of "1.0.0-beta.2+abc", as numbers. */
const parts = (version: string): number[] => (version.split(/[-+]/)[0] ?? "").split(".").map(Number);
/** "1.0.0-beta.2" comes before "1.0.0"; "+abc" (a build) changes nothing. */
const prerelease = (version: string): boolean => /^[^+]*-/.test(version);

/** `a` is a later version than `b` ("0.10.0" after "0.9.2"). */
export function newer(a: string, b: string): boolean {
  const [x, y] = [parts(a), parts(b)];
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    const [m, n] = [x[i] ?? 0, y[i] ?? 0];
    if (m !== n) return m > n;
  }
  return prerelease(b) && !prerelease(a);
}

/**
 * The releases up to `current` (when known), newest first, and after `from`
 * when given: the versions skipped since the one used last.
 */
export function shown<R extends { version: string }>(releases: R[], current: string | null, from: string | null): R[] {
  return releases
    .filter((r) => (current === null || !newer(r.version, current)) && (from === null || newer(r.version, from)))
    .sort((a, b) => (newer(a.version, b.version) ? -1 : newer(b.version, a.version) ? 1 : 0))
    .slice(0, MAX_SHOWN);
}
