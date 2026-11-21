# Roadmap

What is planned beyond the current version. Order within a section is not a commitment.

## v1 (in progress)

- [x] Decision: Tauri 2, single codebase for macOS, Windows and Linux
- [x] macOS island spike (ADR 0001): transparent always-on-top panel, 0% idle
      CPU, ~61 MB, and a click never steals focus
- [x] Live Claude Code sessions in the island (states, ticker, diff stats)
- [x] Approvals (Allow / Deny / Always) bound to the exact pending request
- [x] AskUserQuestion answers from the island
- [x] Safe hook installer (dated backup, merge, diff, explicit confirm)
- [x] Always rules: persisted, re-validated on load, never matching a compound
      shell command; reviewed and removed in Settings
- [x] Settings window: connect / disconnect Claude Code, Always rules, quit
- [x] Edit / Write approval cards show the exact diff before you allow it
- [x] The island stays folded into a handle until a card needs the user; hover shows the sessions
- [x] Packaging: `npm run bundle` embeds the relay; a tag builds dmg / msi /
      exe / AppImage / deb / rpm and drafts a release with checksums
      (verified on macOS; Windows and Linux installers get their first real
      run with the first tag)
- [ ] First tagged release (v0.1.0): check the Windows and Linux installers
- [x] Signed auto-updates, opt-in (ADR 0004)
- [x] GitHub integration (PRs, requested reviews, CI status), read through `gh` — first service pill
- [x] Clicking anywhere outside the island folds it, even with a card waiting;
      the folded island shows that something waits, and hover brings it back
- [x] GitHub, several accounts: every account signed in to `gh`, each in its
      own group; "Sign in to GitHub" (gh's browser login — no token to create
      or paste) and "Install GitHub CLI" when `gh` is missing (ADR 0005)

## v2+

- **Email in the island** — read, filter and act on mail without leaving your flow
- **Network sentinel** — notify when the connection degrades, goes offline or recovers
- **Resource sentinel** — alerts on abnormal RAM and disk usage
- **Cortex mascot** — an optional persona on top of the theme system; the sober
  minimal island stays the default. Comes with the personalization options:
  island position (top center or any corner of the screen) and idle looks
  chosen in Settings (the handle, the Cortex icon, a mini bar with counters,
  the mascot)
- **GitHub without `gh`** — Cortex's own "Sign in with GitHub" (OAuth device
  flow, token in the OS keychain) for people who do not want the GitHub CLI
- More agents: Codex CLI, Gemini CLI (the socket protocol is already generic)
- Chat with AI providers (API key or through the CLI subscription you already pay)
- More service pills: Vercel, n8n, …
- Team / enterprise features (open-core): central visibility across machines,
  centralized approval policies

## Non-goals

- No Cortex server, no account, no telemetry. The app is local by design.
- No Electron. The resource budget is part of the product.
