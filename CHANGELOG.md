# Changelog

All notable changes to Cortex are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com) and the project
follows [Semantic Versioning](https://semver.org).

## [Unreleased]

### Added
- **What's new.** After an update, "Update complete" grows into the
  version's news: a card with a picture and a sentence for each, the fixes
  folded, and the versions you skipped below it. It stays until **Let's get
  to work** or the X, and never takes the keyboard; a version with fixes only
  offers them under a small **What changed**. Settings → About → **What's
  new** opens the same list, in the app's language. (#180)

### Fixed
- **A new review request always speaks** (#179). It rings blue once and says
  so, even when the island is amber or red; the color stays the most urgent
  one's. Requests that come together get one phrase and one sound ("3 new
  reviews · #1258, #1257 and #1248", or "ana asked for 3 reviews"), after a
  more urgent phrase and once a card in view is answered, and the pull
  request count hops.
  - **The Reviews tab.** Beside Notices and Atlantis, every pull request
    waiting for your review, of every account: the new ones on top, marked
    NEW, then the one waiting the longest, with how long. A click opens it on
    GitHub. Opening the list over the phrase shows this tab.
  - **Each notice ringed in its color** in the list.
- **The island stands at the left and right edges** (#183). Folded, it is a
  standing pill: the mascot on top, the counts under it. A phrase, and the
  pink summary, come in a balloon beside it, in the notice's color; open, the
  panel grows from the edge as before. The bottom corners stay as they were.
- **Drag the island along its edge** (#184): sideways on top, up and down at
  the left and right. The drag starts once the pointer moves 6 points pressed,
  so a click stays a click, and the island stays folded while it moves. Near
  the middle it snaps back; only there it wraps the camera. Cortex remembers
  where it went; another position starts in the middle, and Settings →
  Island → Position has **Center**.
- Three texts in Settings read more clearly. In Portuguese, "Abrir ao ligar"
  becomes "Abrir ao iniciar a sessão" and the updates note says the option must
  be on; in both languages, the pink summary says that of what came while
  Cortex was closed, only GitHub's notices are shown.

## [0.9.1] - 2026-11-26

### Changed
- **Atlantis, live.** While Atlantis runs on your pull requests, or on those
  that ask for your review, the island shows Atlantis's sign beside the pull
  request count: a round icon, a ring turning around it, and how many run.
  - **The Atlantis tab.** A click on the sign, or on an Atlantis notice,
    opens the Atlantis tab, and the island stays open until you click
    outside it.
  - **One card per pull request.** Each card shows the accounts it touches
    (production in red) and the five steps, Prepare → Plan → Approval →
    Apply → Merge, with the current one turning and its time.
  - **What runs now.** An open card says what runs now, with its live log,
    or what it waits for. It also shows the plan's summary, and each project
    with its log.
  - **A turquoise of its own.** A plan of yours ready, or applied, now turns
    the island Atlantis's turquoise, a sixth color in Settings → Notices &
    sounds. It plays the green's sound until its own is picked.
  - **No more gray rows.** Runs still going no longer show as gray rows at
    the end of the list. (#173)

### Fixed
- The island no longer stutters while it opens or folds, as right after a
  click on a notice: a redraw on the way (every agent event is one) stopped
  it for a frame or more, and now it keeps moving. (#171)

## [0.9.0] - 2026-11-25

### Added
- **English and Portuguese.** Cortex speaks Portuguese when your system does,
  and English otherwise; Settings → General → Language picks one for good.
  Every window changes at once, the menu-bar item too, and the Windows
  setup (`.exe`) follows the system's language. (#154)

## [0.8.0] - 2026-11-25

### Added
- **Atlantis plans on the island.** For your pull requests and the ones that
  ask for your review, the island follows each Atlantis plan to its apply:
  red when a plan destroys or replaces something (in place of "A PR has
  critical changes"), fails, or its apply fails; green when your plan only
  adds or changes, and when it is applied. Opening a line in the list shows
  the summary: the counts (`+` add, `~` change, `±` replace, `−` destroy), one
  sentence, alerts for data at risk, production and drift, and the changes
  grouped by project and type, with links to the pull request and the full
  plan. Several pull requests at once each get a line, the most urgent open;
  plans still running, or waiting on another pull request's lock, show in
  gray without a sound. Production is a `production`, `prod`, `prd`,
  `infraestructure` or `network` folder right before the region. (#161)

## [0.7.0] - 2026-11-25

### Added
- **While you were away.** Nothing rings or speaks while nobody is looking:
  the screen locked, ten minutes without the mouse or the keyboard, or the
  computer asleep. Coming back, if something came meanwhile, the island turns
  light pink with a summary (one count per color) and plays a soft sound
  once; it stays until **See all** (the list of notices) or **Dismiss**. A
  card waiting for an answer comes first. Opening Cortex does the same for
  the GitHub notices that came while it was closed. (#157)

## [0.6.0] - 2026-11-24

### Changed
- **Settings is a portal.** A sidebar with one page per subject: Overview,
  Agents, Island, Notices & sounds, GitHub, General and About. The Overview
  shows the state of everything at a glance, and a click opens its page.
  Agents already shows where creating your own agents will go. Nothing a
  setting does has changed, only where it lives. (#151)
- **The first launch plays out.** The island appears only once the welcome
  screen ends (at once if an agent is waiting for an answer), and a new
  install then opens Settings with **Get started**: connect Claude Code and,
  if you want, GitHub. (#152)
- **Quicker GitHub notices.** Pull requests are checked about every 15
  seconds instead of every minute, so an approval or a review request shows
  almost at once. Once a minute while GitHub keeps failing, and on macOS
  while the screen is locked. Failed actions still follow GitHub's own pace,
  about a minute. (#159)
- **Updates are checked every minute** while Cortex is being tested, instead
  of every 6 hours; it goes back to 6 hours afterwards. (#158)

## [0.5.0] - 2026-11-24

### Added
- **A lively mascot.** Its eyes (and eyebrows) follow your mouse, it blinks,
  looks around and naps after 10 minutes with nothing going on; a click
  squashes it with a smile, three quick ones make it dizzy; when the island's
  color rises it smiles for green and shakes for red. On by default; Settings → Island turns
  it off, and Reduce motion keeps it from jumping. It costs next to nothing
  (about 0.2% of one core while you move the mouse) and on macOS it stops
  while the screen is locked. (#145)
- Four new mascots in Settings → Island: Skull, Ninja, Bunny mochi and Samurai
  mochi. (#144)
- **Clear all** on top of the list of notices, when two or more are standing:
  one click clears them all. (#150)

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
