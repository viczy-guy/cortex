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
- `docs/` — spec, agent protocol, ADRs; `docs/visao/` is the product vision
  and phases, in Portuguese. `ROADMAP.md` — what is planned.

## Rules

- **Business logic lives in `cortex-core`**, never in the UI or in `src-tauri`
  glue. If it can be a pure function, it is one, with tests.
- **No new dependencies** (Rust or npm) without being asked. Prefer 50 lines of
  code over a crate. Never add git dependencies.
- **The hook never blocks the agent**: any failure in `cortex-hook` means exit 0
  with no output.
- **Every text the user sees is in English and Portuguese** (#154): add its
  key to both tables in `src/shared/i18n.ts` and draw it with `t()`;
  `npm run check` fails when a table misses a key. Atlantis resource names
  are in `KINDS` (`src/island/plans.ts`), both languages too. Rust draws only
  the menu-bar item and the Settings title (`src-tauri/src/language.rs`); a
  fixed message it sends to the UI gets its Portuguese in `FROM_RUST` or
  `FROM_RUST_PATTERNS` (`src/shared/i18n.ts`), shown through `fromRust()`.
- **Hook payloads are untrusted**: validate in Rust; the UI sets dynamic data
  with `textContent` only (use `el()` from `src/shared/dom.ts`), never innerHTML
  (ESLint rejects it).
- **Never write to the user's files** (e.g. `~/.claude/settings.json`) outside
  the reviewed installer flow in `src-tauri/src/hooks.rs`.
- Platform code is `cfg`-gated and must compile on macOS, Linux and Windows.
- Don't restyle or restructure what you were not asked to touch. Keep diffs small.
- Comments state constraints the code can't show — not what the next line does.

## How to work

- Read the code you are about to change, and its tests, before editing. Match
  the style already in the file.
- Smallest diff that does the job. Findings outside the task go in your final
  summary, not in the diff.
- Verify, don't assume. If a check cannot run in your sandbox (the
  `cortex-hook` socket test needs to bind a socket), say so explicitly — never
  report it as passed.
- If the task is ambiguous or looks wrong on anything touching security or
  user files, stop and say so in the summary instead of guessing.

## Patterns in this codebase

- **Time is an argument.** Core functions take `now_ms: u64`; nothing in
  `cortex-core` reads a clock. That is what makes the tests deterministic.
- **Untrusted input.** Parse leniently on unknown fields, strictly on the ones
  we use; cap every size (see the `MAX_*` constants); invalid input is dropped
  or `None`, never a panic.
- **Errors.** Small enums with a manual `Display` in core; `Result<T, String>`
  at the Tauri command boundary. No `unwrap`/`expect` outside tests, except on
  invariants (a poisoned mutex).
- **File writes.** A private helper takes the path explicitly and writes via
  temp file + rename; the public function only resolves the real path. Tests
  run against a temp directory, never the real home.
- **UI.** State comes from Rust snapshots; TypeScript holds view state only.
  DOM is built with `el()` / `button()`; colors and sizes come from
  `src/theme/tokens.css`, never hardcoded.
- **Tests** ship with the change and are named as sentences
  (`a_moved_binary_updates_entries_in_place`).
- **Two places know which events block:** `HookEvent::is_blocking` in
  `cortex-core` and `blocking_kind` in `cortex-hook`. Change both together.

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
