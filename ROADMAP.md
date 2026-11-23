# Roadmap

What is planned beyond the current version. The order of the phases is the
plan; the order inside a phase is not a commitment. Each phase is a
[milestone](https://github.com/viczy-guy/cortex/milestones), and the full
picture, with drawings, is in [docs/visao](docs/visao/README.md) (in Portuguese).

## App track

- [x] **A0 · v0.2.0 — Consolidation.** Hook endpoint with limits; every release
      installs the Windows and Linux packages on clean CI runners.
- [ ] **A1 · v0.2.x — Maintenance.** Spec in step with the app, two copies
      running at once, an easy way to quit a stuck app, current GitHub Actions.
- [ ] **A2 · v0.3.0 — New island.** The shape of the MacBook camera cutout,
      reacting to the mouse; choice of screen and five positions; up to four
      agents side by side; mascot and border take the state color (green done,
      amber needs you, red danger) until you act; a mascot of your choice; one
      sound per kind of notice; an opening screen; optional launch at login.
- [ ] **A3 · v0.4.0 — Updates itself, new name.** A stable version installs
      with no click when nothing is waiting; updating, done and error screens;
      a test channel before the stable one; new features ship switched off;
      the new product name, migrating existing installs.
- [ ] **A4 · v0.5.0 — Ask from the island.** A prompt box in the island: pick
      the agent, the model and the folder. It runs through the CLIs you already
      have installed and signed in; everything the agent wants to do still goes
      through the same approval cards.
- [ ] **A5 · v0.6.0 — More agents.** Codex CLI and Gemini CLI; showing when one
      agent calls another; Always rules per agent and per folder; a red card
      with no Always for destructive actions.
- [ ] **A6 · v0.7.0 — Act on pull requests.** A notice when someone requests
      your review; the agent's analysis in the card; approve or request changes
      from the island, with your own GitHub account.
- [ ] **A7 · v0.8.0 — Your agents.** A recipe per agent: model, folder,
      instructions, tools and knowledge sources.
- [ ] **A8 · v1.0 — Connected app.** An optional account; notices and approvals
      for agents that run on a server; Windows and Linux installers tried on
      real machines.

## Platform track

A separate product for teams, planned for after the app track: agents in
folders with their tools and knowledge, users and devices, indicators, and
approvals that reach the coordinator's island. Phases P0 to P5 are described in
[docs/visao](docs/visao/README.md#fases). Nothing is built yet, and no code is
written before its spec is approved.

## Ideas

No phase, no commitment.

- **Email in the island** — read, filter and act on mail without leaving your flow
- **Network sentinel** — notify when the connection degrades, goes offline or recovers
- **Resource sentinel** — alerts on abnormal RAM and disk usage
- **GitHub without `gh`** — Cortex's own "Sign in with GitHub" (OAuth device
  flow, token in the OS keychain) for people who do not want the GitHub CLI
- Chat with AI providers (API key or through the CLI subscription you already pay)
- More service pills: Vercel, n8n, …
- Diff of the edits already applied, per file, in the session detail

## Shipped

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
      (every release installs the Windows and Linux packages on clean CI
      runners and starts the app before the draft is created)
- [x] First release, v0.1.0 (tested by hand on macOS)
- [x] Signed auto-updates, opt-in (ADR 0004)
- [x] GitHub integration (PRs, requested reviews, CI status), read through `gh` — first service pill
- [x] Clicking anywhere outside the island folds it, even with a card waiting;
      the folded island shows that something waits, and hover brings it back
- [x] GitHub, several accounts: every account signed in to `gh`, each in its
      own group; "Sign in to GitHub" (gh's browser login — no token to create
      or paste) and "Install GitHub CLI" when `gh` is missing (ADR 0005)

## Non-goals

- The app needs no server and no account, and sends no telemetry. Connecting to
  a team platform is optional, arrives in phase A8 and needs a new decision
  record in place of ADR 0002.
- No Electron. The resource budget is part of the product.
