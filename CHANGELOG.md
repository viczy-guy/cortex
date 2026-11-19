# Changelog

All notable changes to Cortex are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com) and the project
follows [Semantic Versioning](https://semver.org).

## [Unreleased]

### Added
- GitHub account groups and per-account visibility, browser sign-in from
  Settings, and GitHub CLI installation through Homebrew or winget.
- A click in another app folds the island, even over a waiting card; the
  handle turns amber until the card is answered, and hovering brings it back.
- Opt-in GitHub pill and card for open PRs, requested reviews and CI status,
  read through the signed-in `gh` CLI every 3 minutes; no token stored.
- Project foundation: Cargo workspace (`cortex-core`, `cortex-hook`, `cortex-app`),
  Tauri 2 application shell, CI on the three platforms.
- The island: live Claude Code sessions, approvals (Allow / Deny / Always),
  `AskUserQuestion` answers, and a hook installer that shows the exact diff and
  keeps a dated backup.
- Settings window: connect or disconnect Claude Code, review and remove Always
  rules, quit.
- Always rules persist across restarts and are re-validated when loaded.
- Edit and Write approval cards show the exact diff (up to 300 lines) before
  you allow them; redraws keep your scroll position.
- The island stays folded into a small handle until a card needs you; hovering
  it shows the live sessions.
- Installers (`npm run bundle`) with the relay embedded; a release workflow
  that builds them from a tag and drafts a release with SHA-256 checksums.

### Fixed
- A question longer than 300 characters, or two questions with the same text,
  could send the answer back under the wrong key or leave the card stuck;
  such requests now go to the terminal, and questions up to 1000 characters
  are shown whole.
- Installing or removing the hooks through a `settings.json` that is a link
  (dotfiles) writes the linked file and keeps the link, and the temporary file
  is private from the start.
- A question card no longer vanishes before you can answer it: the island
  waits up to 110 s (was 50 s). Settings flags hooks installed by an older
  version and offers "Update hooks…".

### Security
- An Always rule for a shell command covers one program and never a command
  line that chains, pipes, redirects or substitutes, nor a command runner
  under any path or case (`/bin/bash`, `BASH`); such commands always ask. A
  card that shows no target (an MCP tool, say) offers no Always.
- The island's web view can no longer move or resize windows.
- Each window can call only the app commands it uses: the island cannot quit,
  uninstall hooks or change rules, and Settings cannot answer approval cards.
