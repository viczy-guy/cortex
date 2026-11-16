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
- [ ] Full diff view per file, persisted Always rules, hook uninstall from the UI
- [ ] Settings window, island behavior, packaging (dmg / msi / AppImage / deb / rpm)
- [ ] Signed auto-updates (project key, free) or update notice with download link
- [ ] GitHub integration (PRs, requested reviews, CI status) — first service pill

## v2+

- **Email in the island** — read, filter and act on mail without leaving your flow
- **Network sentinel** — notify when the connection degrades, goes offline or recovers
- **Resource sentinel** — alerts on abnormal RAM and disk usage
- **Cortex mascot** — an optional persona on top of the theme system; the sober
  minimal island stays the default
- More agents: Codex CLI, Gemini CLI (the socket protocol is already generic)
- Chat with AI providers (API key or through the CLI subscription you already pay)
- More service pills: Vercel, n8n, …
- Team / enterprise features (open-core): central visibility across machines,
  centralized approval policies

## Non-goals

- No Cortex server, no account, no telemetry. The app is local by design.
- No Electron. The resource budget is part of the product.
