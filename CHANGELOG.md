# Changelog

All notable changes to Cortex are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com) and the project
follows [Semantic Versioning](https://semver.org).

## [Unreleased]

## [0.5.0] - 2026-11-23

### Added
- **A lively mascot.** Its eyes follow your mouse, it blinks, looks around
  and naps after 10 minutes with nothing going on; a click squashes it with
  a smile, three quick ones make it dizzy; when the island's color rises it
  smiles for green and shakes for red. On by default; Settings → Island turns
  it off, and Reduce motion keeps it from jumping. It costs next to nothing
  (about 0.2% of one core while you move the mouse) and on macOS it stops
  while the screen is locked. (#145)
- Four new mascots in Settings → Island: Skull, Ninja, Bunny mochi and Samurai
  mochi. (#144)

### Changed
- The glasses and mustache mascot has eyes behind the glasses and eyebrows. (#144)

## [0.4.3] - 2026-11-23

### Changed
- "Your PR #128 (Short title) was approved!" is blue, like a review request,
  and rings the blue sound; it was green. (#141)

## [0.4.2] - 2026-11-23

### Changed
- Cortex checks GitHub about once a minute instead of every 3 minutes, so a
  review request, an approval or a failure reaches the island sooner. The
  notices of failed actions are read no more often than GitHub asks. (#138)

## [0.4.1] - 2026-11-23

### Changed
- When someone approves a pull request of yours (not a draft), the island
  says so in green: "Your PR #128 (Short title) was approved!", at its next
  GitHub check (every 3 minutes), even while the checks still run. It takes
  the place of "Your PR is ready"; if the checks then fail, "A PR failed!"
  stands beside it. (#135)

## [0.4.0] - 2026-11-23

### Added
- Notices from GitHub, with GitHub on in Settings: the island turns blue when
  someone asks for your review, green when a pull request of yours is ready
  (approved, checks passing), red when one of yours fails its checks, when an
  action you started fails, or when an Atlantis plan on a pull request
  destroys something. (#131)
- Each notice says what happened in a short phrase ("A PR needs your
  review", "An action failed!", "An agent finished"…) for 5 seconds, with
  where it happened, then the island folds back in its color. Hovering lists
  every notice standing; a click on one clears it, and a GitHub one also
  opens on GitHub.
  Blue has its own sound, softer than the others. (#131)

### Changed
- In the session cards, the "working" dot is gray, as on the island: blue now
  means a review was requested. (#131)

## [0.3.2] - 2026-11-23

### Added
- Updating shows a screen: "Updating…" with the versions, then "Update
  complete, let's get to work?" when the new version opens. If the install
  fails, the screen says so and offers **Try again** or **Continue without
  updating**. Before, the island just vanished and came back. (#127)

### Changed
- The island keeps its color when you hover it. Green, red and an agent's
  own "waiting for you" go back to gray when the agent moves on or when you
  click: the island clears them all, a session's card clears that session.
  (#126)

## [0.3.1] - 2026-11-23

### Changed
- The island opens and folds smoothly, growing from the edge it hangs on,
  instead of jumping to its new size. (#118)
- The opening screen takes about 6 seconds instead of 2. (#117)

### Fixed
- macOS: the island could stay invisible after an update, when the system
  part that draws it stopped answering. Cortex now notices and restarts
  itself once. (#115)
- macOS: opening Cortex while it runs (Finder, Launchpad, Spotlight) opens
  Settings; before, nothing seemed to happen. (#115)
- The app icon and the macOS menu-bar icon are the mascot's eyes, the icon
  the maintainer chose; 0.3.0 shipped the dot with rings by mistake. (#116)
- The three sounds are again the ones chosen on the vision page: plucked
  notes at their own volumes, instead of flat, organ-like tones. (#117)

### Security
- `cookie` 0.18.3, for RUSTSEC-2026-0594 (repeated nonces in private cookies
  after a fork). It comes in through Tauri; Cortex does not use private
  cookies.

## [0.3.0] - 2026-11-23

### Added
- Cortex opens when you log in. **It is on from the first launch of this
  version**; turn it off in Settings → Startup. (#81)
- A new island: a black notch at the top of the screen, the shape of the
  MacBook camera cutout, with a mascot. The mascot and the border take one
  color per meaning: amber while an agent waits for you, green when one
  finished, red when one failed. On a MacBook with a camera cutout, the
  top-center island places its mascot and controls on either side. (#77)
- Up to four agents side by side, each with its own tag and state dot, the
  one that needs you first; cards come one at a time with the next in view.
  (#78)
- A mascot of your choice, from a gallery of eight in Settings → Island, or
  none. (#27)
- An opening screen when you open Cortex: a growing dot, "Welcome", then
  your mascot. About two seconds; a click skips it. (#80)
- Choose the island's screen and one of five positions (top center, either
  side, either bottom corner) in Settings → Island. (#43)
- A sound for each kind of notice: an agent done, an agent waiting for you,
  an agent failed. On by default; mute and samples in Settings → Island. (#79)

### Changed
- A new app icon: a dot with two rings, the dot that opens the island.
  (#101)

## [0.2.1] - 2026-11-23

### Added
- macOS: a menu-bar item with Settings… and Quit Cortex. It quits the app
  even when the island stops responding. (#76)

### Fixed
- Opening a second copy of Cortex (say, from a mounted `.dmg` while the
  installed one runs) no longer leaves the first one deaf to its agents. The
  new copy sees the running one and quits. (#73)

### Security
- `url` updated to 2.5.9, which fixes a crash on deeply nested `blob:` URLs
  (RUSTSEC-2026-0592). Cortex does not parse URLs it receives from agents,
  so this was not reachable through a hook.

## [0.2.0] - 2026-11-21

### Fixed
- Settings says so when removing an Always rule could not be saved, instead
  of showing it as removed until the next launch.
- A hook of yours whose command only mentions `cortex-hook` is no longer
  taken for Cortex's own when hooks are installed or removed.
- An event just under the size limit no longer leaves the agent waiting for
  a card that never appears: it goes straight to the terminal.

### Security
- The hook endpoint reads lines with a size limit, caps connections, sessions
  and waiting cards, and assigns request ids itself. On Windows only the
  pipe's owner can open it.
- The relay gives up after one total deadline on every platform.
- An update download that stalls is abandoned after 15 minutes.

### Changed
- With sessions on the board Cortex no longer wakes up every 2 seconds: it
  sleeps until the next session or card is due.

## [0.1.1] - 2026-11-21

### Added
- Settings → About: "What's new" opens the release notes of the installed
  version, and a local test build shows the commit it was built from.

## [0.1.0] - 2026-11-20

### Added
- Signed, opt-in updates: Cortex asks once whether to check for updates,
  downloads a new release in the background, verifies its signature and
  installs it only when you click "Update and restart" (Linux: AppImage).
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
- Account names in the GitHub card were black on the dark island and hard
  to read; text without its own color now uses the island's text color.
- The folded island no longer repaints on every agent event, which with the
  screen locked kept WebKit busy at a fraction of a percent of CPU.
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
