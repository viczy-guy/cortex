# Changelog

All notable changes to Cortex are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com) and the project
follows [Semantic Versioning](https://semver.org).

## [Unreleased]

### Added
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
- A question card no longer vanishes before you can answer it: the island
  waits up to 110 s (was 50 s). Settings flags hooks installed by an older
  version and offers "Update hooks…".

### Security
- An Always rule for a shell command covers one program and never a command
  line that chains, pipes, redirects or substitutes; such commands always ask.
- The island's web view can no longer move or resize windows.
