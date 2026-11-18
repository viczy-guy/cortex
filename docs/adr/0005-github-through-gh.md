# ADR 0005 — GitHub pill: read through the user's `gh` CLI, never store a token

**Status:** accepted · 2026-11-18

## Context

The first service pill shows the user's open pull requests (with CI status and
review decision) and the pull requests waiting for their review. That needs an
authenticated call to the GitHub API. The obvious design — a personal access
token pasted into Settings, stored in the OS keychain, sent by an HTTP client
inside Cortex — adds two dependency trees (HTTP/TLS and keyring), a secret
Cortex is responsible for, and a token-management UI.

Most people who run coding agents already have the GitHub CLI signed in, often
with more than one account (personal and work).

## Decision

1. **Cortex asks `gh` for the data**: `gh api graphql` with one fixed query.
   Cortex never stores a token; `gh` keeps using its own credential storage
   (the OS keychain on macOS and Windows).
2. **Every account signed in to `gh` can be shown**, each in its own group.
   Accounts are listed with `gh auth status --json hosts` (no tokens). `gh api`
   always uses the active account, so for any other account Cortex reads that
   account's token with `gh auth token --user <login>` and hands it to that one
   `gh api` call through its environment: in memory for the duration of the
   call, never written or logged. Cortex never switches `gh`'s active account.
3. **No token for the user to create or paste.** When `gh` is missing, Settings
   offers to install it (Homebrew on macOS, winget on Windows; Linux shows the
   command). "Sign in to GitHub" runs `gh`'s own browser login and shows its
   one-time code; signing in a second account makes it `gh`'s active one, so
   Cortex switches back to the previous account right after.
4. **Off by default.** The pill is turned on in Settings. This is a network
   call the user asks for (ADR 0002); while it is off, Cortex runs nothing.
5. **Polling, slow and only while on:** one query per enabled account every 3
   minutes, plus "Refresh now". No timers at all while off.
6. `gh` is found on `PATH` and in the usual install locations (GUI apps on
   macOS do not inherit the shell `PATH`). If it is missing or signed out,
   Settings says exactly what to do.
7. The response is untrusted input: parsed in `cortex-core`, capped in size,
   and a link is opened only if it is an `https://github.com/` URL from the
   latest fetched data.

## Consequences

- No new dependencies; nothing secret at rest on Cortex's side.
- People who do not want the GitHub CLI cannot use the pill yet. A
  Cortex-owned sign-in (OAuth device flow, token in the OS keychain) stays
  possible later as a second source behind the same summary type; it needs an
  HTTP/TLS client, a keyring crate and a registered OAuth App.
- The in-app sign-in depends on the text `gh auth login` prints (the one-time
  code). If a `gh` release changes it, Cortex falls back to showing the
  command to run in a terminal.
- GitHub Enterprise hosts are out of scope for v1 (links must be github.com).
