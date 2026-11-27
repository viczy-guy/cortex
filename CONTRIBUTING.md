# Contributing

Issues and PRs are welcome. The short version of how this codebase stays clean:

## Ground rules

- **Business logic lives in Rust** (`crates/cortex-core`), never in the UI.
  The TypeScript layer renders state and forwards clicks — nothing else.
- **Dependencies are a liability.** Prefer writing 50 lines over adding a crate
  or npm package. New dependencies must be established, actively maintained and
  justified in the PR description. CI fails on known vulnerabilities.
- **Hooks never block the agent.** Any change to `cortex-hook` must preserve the
  instant-exit path when the app is not running.
- **Untrusted input stays untrusted.** Hook payloads are schema-validated in Rust;
  the UI never interpolates external data as HTML.
- Existing look and behavior don't change without being asked; pill/agent IDs are
  stable contract values once released.

## Workflow

- `cargo fmt`, `cargo clippy` (warnings fail CI), `cargo test` before pushing.
- Conventional commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `ci:`).
- Big decisions get a one-page ADR in `docs/adr/`.
- Every release updates `CHANGELOG.md`, and adds its entry on top of
  `src/splash/news.ts` (What's new, #180): one to four cards in English and
  Portuguese, each with its release-note picture squeezed to about 50 KB when
  it has one, or only `fixes` when there is nothing to see. The oldest entry beyond five goes,
  with its pictures.

## Dev setup

```bash
npm ci
npm run tauri dev     # run the app
```

Only one Cortex serves an endpoint: with the installed app running, the dev
build quits at launch. Give it its own, and the same to `scripts/demo.sh`:
`CORTEX_SOCKET=/tmp/cortex-dev.sock npm run tauri dev`.

```bash
cargo test            # core + hook tests
npm run check         # typecheck and lint the UI
```
