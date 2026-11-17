# ADR 0005 — GitHub pill: read through the user's `gh` CLI, never hold a token

**Status:** proposed · 2026-11-17

## Context

The first service pill shows the user's open pull requests (with CI status and
review decision) and the pull requests waiting for their review. That needs an
authenticated call to the GitHub API. The obvious design — a personal access
token pasted into Settings, stored in the OS keychain, sent by an HTTP client
inside Cortex — adds two dependency trees (HTTP/TLS and keyring), a secret
Cortex is responsible for, and a token-management UI.

Most people who run coding agents already have the GitHub CLI signed in.

## Decision

1. **Cortex asks `gh` for the data**: `gh api graphql` with one fixed query.
   Cortex never sees, stores or sends a token; `gh` keeps using its own
   credential storage (the OS keychain on macOS and Windows).
2. **Off by default.** The pill is turned on in Settings. This is a network
   call the user asks for (ADR 0002); while it is off, Cortex runs nothing.
3. **Polling, slow and only while on:** one query every 3 minutes, plus
   "Refresh now". No timers at all while off.
4. `gh` is found on `PATH` and in the usual install locations (GUI apps on
   macOS do not inherit the shell `PATH`). If it is missing or signed out,
   Settings says exactly what to run.
5. The response is untrusted input: parsed in `cortex-core`, capped in size,
   and a link is opened only if it is an `https://github.com/` URL from the
   latest fetched data.

## Consequences

- No new dependencies; nothing secret on Cortex's side.
- Users without `gh` must install it to use the pill. A keychain-stored token
  can be added later as a second source behind the same summary type, if users
  ask for it.
- The pill follows whichever account is active in `gh` (`gh auth switch`).
- GitHub Enterprise hosts are out of scope for v1 (links must be github.com).
