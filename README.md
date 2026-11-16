<div align="center">

# Cortex

**A lightweight companion that lives at the top of your screen and keeps an eye on
your AI coding agents — on macOS, Windows and Linux.**

Watch your Claude Code sessions live, approve permissions and answer questions
with one click, without leaving what you are doing.

</div>

---

> **Status: early development.** Cortex is being built in the open. Nothing here
> is ready for daily use yet — follow the [roadmap](ROADMAP.md).

## Why

AI coding agents work for minutes at a time and then wait for you: a permission
to run a command, a question with options, a finished task. Cortex keeps that
conversation at the top of your screen in a small, quiet island — so you never
babysit a terminal again, and never miss the moment you are needed.

Inspired by the excellent [coucou](https://github.com/Louis-CFM/coucou) (MIT),
rebuilt from scratch as a single cross-platform codebase with its own identity.

## Principles

- **Never block the agent.** If Cortex is not running, the hook exits instantly.
- **Private by design.** No telemetry, no account, no server. Secrets live in the
  OS vault (Keychain / Credential Manager / Secret Service). The app only talks
  to services you configure.
- **Light or it doesn't ship.** 0% CPU while hidden, low memory, tiny binary.
  Tauri 2 (Rust core + system WebView), not Electron.
- **Minimal dependencies, audited in CI.** A known vulnerability fails the build.

## Architecture

```
Claude Code ──hooks──▶ cortex-hook (tiny Rust relay)
                           │ newline-delimited JSON
                           ▼
            Unix socket / named pipe (your user only)
                           │
                 ┌─────────▼─────────┐
                 │  Cortex (Tauri 2) │
                 │  cortex-core:     │
                 │  sessions, diffs, │
                 │  approval queue   │
                 └─────────┬─────────┘
                           ▼
                 Island UI (transparent,
                 always-on-top, never steals focus)
```

| Crate | Role |
|---|---|
| `crates/cortex-core` | Domain logic: events, session state machine, approvals, diff stats. Pure Rust, no Tauri, fully unit-tested. |
| `crates/cortex-hook` | The relay agents call on every hook event. Single static binary, exits instantly when the app is absent. |
| `src-tauri` (`cortex-app`) | The Tauri shell: socket server, state, windows, OS integration. |
| `src/` | The island UI: strict TypeScript, no business logic, minimal npm deps. |

Any agent can talk to Cortex: send newline-terminated JSON with a `cortex_agent`
field to the socket. See [docs/AGENTS.md](docs/AGENTS.md).

## Build from source

Requirements: [Rust](https://rustup.rs) stable, Node 20+.

```bash
git clone https://github.com/viczy-guy/cortex.git
cd cortex
npm ci
npm run tauri dev
```

Linux also needs the WebKitGTK development packages — see
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

## License

- **Code:** [MIT](LICENSE).
- **Name, icon and future mascot/sounds:** all rights reserved — see
  [LICENSE-ASSETS.md](LICENSE-ASSETS.md). Forks are welcome under their own name.
