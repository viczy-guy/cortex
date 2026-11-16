# Cortex — guide for AI coding agents

Cortex is a Tauri 2 desktop app (macOS, Windows, Linux): a small island at the
top of the screen that shows AI coding agent sessions and lets the user approve
permissions and answer questions. Read `docs/SPEC.md` before changing behavior.

## Where things are

- `crates/cortex-core/` — domain logic (events, sessions, approvals, diffs, hook
  merge). Pure Rust: no I/O, no Tauri. Everything here is unit-tested.
- `crates/cortex-hook/` — the relay agents call on every hook event.
- `src-tauri/` — the Tauri shell (`cortex-app`): socket server, state, file I/O,
  OS integration, commands.
- `src/` — the island UI (strict TypeScript, no framework). `src/shared/ipc.ts`
  is the only place the UI calls Rust.
- `docs/` — spec, agent protocol, ADRs. `ROADMAP.md` — what is planned.

## Rules

- **Business logic lives in `cortex-core`**, never in the UI or in `src-tauri`
  glue. If it can be a pure function, it is one, with tests.
- **No new dependencies** (Rust or npm) without being asked. Prefer 50 lines of
  code over a crate. Never add git dependencies.
- **The hook never blocks the agent**: any failure in `cortex-hook` means exit 0
  with no output.
- **Hook payloads are untrusted**: validate in Rust; the UI sets dynamic data
  with `textContent` only (use `el()` from `src/island/dom.ts`), never innerHTML.
- **Never write to the user's files** (e.g. `~/.claude/settings.json`) outside
  the reviewed installer flow in `src-tauri/src/hooks.rs`.
- Platform code is `cfg`-gated and must compile on macOS, Linux and Windows.
- Don't restyle or restructure what you were not asked to touch. Keep diffs small.
- Comments state constraints the code can't show — not what the next line does.

## Before you finish

All of these must pass; warnings are errors:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run check
```

Do not commit, push, or change git configuration — leave the changes in the
working tree and end with a short summary of what you changed and why.
