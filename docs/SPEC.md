# Cortex — behavior spec

Short, normative description of how the app behaves. When code and spec diverge,
one of them is wrong — fix that one.

In one line: Cortex is the local gatekeeper of your coding agents. It shows what
each session is doing and lets you approve and answer without leaving what you
are doing.

## The island

- A transparent, always-on-top, undecorated window. It never steals focus:
  clicking a button must not deactivate the app the user is typing in (macOS:
  non-activating panel).
- The island defaults to the top center of the main screen, flush with the top
  edge. Settings remembers the selected screen and one of five positions: top
  center, left side, right side, bottom-left corner or bottom-right corner.
  Away from the top it is a tab on the edge of the work area and grows inward.
  If the chosen screen disconnects, the island moves to the main screen and
  returns when the chosen screen reconnects.
- At top center on a MacBook display with a camera cutout, the top row matches
  the cutout height and leaves its center, the camera's width, empty: the
  mascot and at most two agent tags on the left; the counter of the others,
  the pull request count and the gear on the right; the "Connect" and update
  chips on a row under the camera; cards below.
- States: `folded` (no card waiting — **0% CPU**, no timers, no animation
  frames) → `bar` (on hover) → `expanded` (session detail, approval/question
  cards) on click, or by itself when a card needs the user.
- Folded, the island is a black notch at the top (the shape of the MacBook
  camera cutout) or a tab at the selected edge. It shows the mascot in the
  middle and, when pull requests wait for the user's review, the pull request
  sign and their count.
  It stays folded while sessions run. Hovering the notch (or clicking it)
  brings the bar back; it folds again shortly after the pointer leaves, and
  once the last card is answered. The island never folds while the hooks are
  missing — that is when "Connect Claude Code" must be seen.
- The bar shows one tag per session (the agent's initials on its own color,
  and a dot in the session's color), at most four, the most urgent first; the
  rest is a counter ("+2"). Then the pull request count, the update chip and
  the gear to Settings.
- One color per meaning, on the mascot and the island's border at once: gray
  while agents work, **amber** while one waits for the user, **green** when
  one finished, **red** when one failed. Amber from a card lasts until the
  card is answered or expires. Green, red and an agent's own "waiting for
  you" notice last until the agent moves on or the user clicks; green and red
  stay even after the session is gone. A click on the island's bar (its
  buttons included) clears them all, a click on a session's card clears that
  session. Hovering opens the island in the color and clears nothing (#126).
  When the color rises, the
  mascot jumps twice, then stays still (not at all with reduced motion).
- Cards come one at a time; the ones after it wait in view below, and the
  bar says "1 of N".
- A click anywhere outside the island folds it, even over a waiting card: the
  notch stays amber until every folded card is answered or expires, and
  hovering brings the cards back. A new card opens the island again. (macOS: a
  global mouse-down monitor, which needs no permission and only wakes the UI
  while a card waits; Windows / Linux: the window's blur, so only after the
  island had focus.)
- The window is always exactly as large as what it draws, so no invisible area
  eats clicks. Only while the island opens or folds (about a third of a
  second, eased, from the edge it hangs on) is the window as large as both
  sizes; with reduced motion the change is instant. The UI cannot move or
  resize windows itself: it asks Rust for a size. Rust fits the window to
  the content within the screen bounds and positions it at the selected edge.
- Theming: all colors/spacing come from CSS tokens under a `data-theme`
  attribute. Personas (e.g. a future mascot) are additive themes, never forks of
  the UI.

Each notice has its own sound, about two seconds long, synthesized note for
note as on the vision page (each note plucked: up in 15 ms, then an
exponential fade), played once per change: a session's state turning done
(green), waiting for the user or a new card (amber), or failed (red). If several come together, the most urgent
plays; a lower one arriving while a higher one plays is not played. Sounds
are on by default and can be muted or sampled in Settings → Island.

## Opening screen

On manual launch, a small centered window shows a growing colored dot,
"Welcome", then the selected mascot and "Let's get to work?" with the app
version at the bottom. It closes after about 6 seconds. A click skips it
(it never takes the keyboard). With reduced motion, it shows the final
frame without animation for 3 seconds. Launches at login (`--autostart`)
skip it. Rust closes the window after 10 seconds if it is still open.

The same window shows the updates (#127):

- **Updating…**: a click on **Update and restart** (island or Settings)
  opens it with the mascot, a moving bar (the package is already downloaded
  and checked: there is no percentage to show) and "0.3.1 → 0.3.2". The
  install starts after about a second, so the screen is seen.
- **Update complete, let's get to work?**: the new copy shows it at its
  first launch, with the mascot green and the version at the bottom, even at
  login or after a recovery restart (the update came from a click, and the
  restart keeps the old arguments). It closes after about 4 seconds, or on a click. The old copy saves its
  version in the preferences (`updated_from`) before installing (on Windows
  the installer ends the process); the new one clears it, and shows nothing
  if the version did not change.
- **Oops, something got in the way.**: the install failed. The mascot turns
  amber, with **Try again** and **Continue without updating**: a failed
  update never keeps the user from working. The package is kept, so the
  island still offers the update. Only this screen can install; the 10-second
  fallback does not close it once its page answered.

## Sessions

One pill per (`cortex_agent`, `session_id`). State machine:

| Event | State |
|---|---|
| `SessionStart` | `idle` |
| `UserPromptSubmit` | `thinking` (prompt in ticker) |
| `PreToolUse` | `working` (tool label in ticker); `waiting_approval` while an `AskUserQuestion` card waits |
| `PermissionRequest` | `waiting_approval` while its card waits; `working` when an Always rule allows it |
| `PostToolUse`, `PostToolUseFailure` | `working`; file edits record name and +N/−M |
| `Notification` | `waiting_input` (message in ticker) |
| `Stop` | `finished`; the pill is removed 5 s later unless another event comes |
| `StopFailure` | `error` |
| `SubagentStart`, `SubagentStop` | a step in the ticker; state unchanged |
| `SessionEnd` | pill removed |
| any other event | keeps the session alive; state unchanged |

The first event from an unknown session creates its pill. A request left to
the terminal (no card shown) does not change the pill.

Sessions stale for 30 min with no events are garbage-collected.

## Approvals

- A `PermissionRequest` shows a card: tool, detail (command / file path /
  notebook path / URL), **Allow / Deny / Always** and "Answer in terminal".
- An `Edit` card shows the exact change and a `Write` card the whole new
  content, as a line diff (at most 300 lines; the rest is counted, and the
  terminal shows it all). Redraws keep the scroll position.
- The decision is bound to the exact request id; anything else (timeout, stale
  id, duplicate) resolves to "no decision" and the terminal takes over.
- The card disappears the moment its connection closes (hook timeout, or the
  user answered in the terminal). The relay waits 110 s (hook timeout 120 s;
  the app's own limits, 115 s and 118 s, only back it up), long enough to read a
  multi-question card; "Answer in terminal" hands it back sooner.
- "Always" stores a rule applied by Cortex, never written into the agent's own
  settings. The card states what the rule covers before the click.
  - Shell commands generalize to their program: Always on `cargo test` allows
    every `cargo …`. Only a single plain command can create or match a rule —
    a line with chaining, pipes, redirection, substitution (`; & | < > $ ( )`
    `{ }`, backticks, backslash, newline), a `VAR=value` prefix, a program
    word with a glob, quotes or `~`, or a command runner (`sudo`, `sh`, `env`,
    `xargs`, …, under any path, case or `.exe` suffix: `/bin/bash`, `BASH`)
    always shows a card and offers no Always.
  - Every other tool must match its detail exactly; a card that shows no
    detail (an MCP tool, say) offers no Always.
  - Rules persist in `rules.json` in the app data directory, are re-validated
    on load, and can be reviewed and removed in Settings.

## Questions

The `PreToolUse` of `AskUserQuestion` renders its options as buttons (single
or multi-select, up to 4 questions, 8 options each). A lone single-choice
question is answered by one click; otherwise "Send answer" enables once every
question has a choice. Answers are validated in Rust against the card's own
options before anything is sent. "Answer in terminal" returns no decision.

The question texts and option labels are the keys of the answer, so a card
appears only when it can show every question whole: at most 1000 characters
per question and 300 per label, no two questions with the same text, and no
invalid question among them. Anything else is left to the terminal.

## Hook installer

When the hooks are missing, the idle island offers **Connect Claude Code**. It
reads `~/.claude/settings.json`, shows the exact line diff of the merge (Cortex
entries only — existing entries and key order are untouched) and writes only
after the user clicks Install: dated backup first
(`settings.json.cortex-backup-YYYYMMDD-HHMMSS`), then an atomic write. If the
file changed between review and install, the install is refused. Invalid JSON
is reported, never "repaired". Hooks point at a stable copy of the relay in the
app's data directory, refreshed on every launch.

## Settings

The gear in the island opens a separate, opaque Settings window:

- **Claude Code** shows hook status and offers reviewed installation or removal,
  with the exact diff and a dated backup before writing.
- **Always rules** lists local rules and allows removing one or all. Removing
  all requires a second click within four seconds.
- **Island** chooses the mascot from a gallery of eight, or none, sets the
  island's screen and position, and controls and samples the sounds.
- **Startup** turns launch at login on or off (see below).
- **GitHub** turns the pull request pill on or off and manages its accounts
  (see [GitHub](#github)).
- **Updates** turns the update check on or off and offers **Update and
  restart** once a verified update is ready.
- **About** shows the app version, a "What's new" link to the notes of that
  version, its local-only policy, and Quit Cortex.

On macOS, Cortex also has a menu-bar item with **Settings…** and **Quit
Cortex**. It is a native menu, so it still quits when the island's web view
does not respond; a background app is not listed in Force Quit.
Opening Cortex while it runs (the Finder, Launchpad, Spotlight) opens
Settings.

WebKit can give up on the process that draws the island and leave the window
empty while the page runs on. So on macOS, 30 seconds after launch and each
time Cortex is opened while it runs, the app asks WebKit for a picture of the
visible island (up to three tries, 15 seconds apart). Two refusals restart
Cortex; the new copy gets `--recovered-at=<time>`, and for ten minutes it shows
no opening screen and does not restart again. A slow picture never counts.
Cards still waiting go back to their agents, which then ask in the terminal.

## Launch at login

On from the first launch, and from then on the user's choice in Settings
stands. The entry runs the app with `--autostart`: a LaunchAgent in
`~/Library/LaunchAgents` on macOS, the current user's `Run` key on Windows, an
XDG autostart entry in `~/.config/autostart` on Linux. No dependency writes
it. A dev build, or an app still on its disk image, never adds one. While on,
each launch rewrites the entry if the app moved.

## GitHub

Opt-in in Settings, off by default. Cortex reads up to 10 github.com accounts
from the GitHub CLI (`gh`). Accounts are shown by default and can be hidden in
Settings. Cortex checks shown accounts every 3 minutes while on (or when
Refresh now is clicked). Off means no polling or timers.

The island pill totals requested reviews, failing own PRs, and open PRs across
shown accounts. Its expanded GitHub card groups requested reviews and own pull
requests by account and shows checks, draft and review status. Each account
keeps its last good data after a fetch error. GitHub does not change when the
island folds.

Settings can run gh's browser sign-in, show its one-time code, and restore the
previous active gh account after adding an account. If gh is missing, Settings
offers to install it with Homebrew on macOS or winget on Windows when available,
or links to the GitHub CLI site. Cortex validates each account token and passes
it only in the environment of that account's single `gh api graphql` child,
without storing or logging it.

Cortex opens GitHub links from the current summaries, the fixed GitHub device
login page, and the GitHub CLI site through the OS with the URL as one argument
and no shell.

## Updates

Opt-in (ADR 0004). On first launch the island asks once, "Check for updates
automatically?"; until the answer is Yes, Cortex makes no update request. The
answer can be changed in Settings → Updates.

While on, Cortex fetches the release manifest (`latest.json` of the latest
published GitHub release) at launch and every 6 hours, and sends nothing but that
request. A newer version is downloaded in the background and verified against
the minisign public key compiled into the app; the signature must also name the
announced version, so a tampered manifest cannot downgrade to an older signed
build. A package that fails verification is discarded and reported in Settings.

A verified update waits for a click on **Update and restart**, in Settings or as
a chip in the island bar, which opens the update screen (see "Opening
screen"). Cortex never restarts on its own, and the chip is not offered (and
both commands refuse) while an approval card is waiting. On Linux,
only the AppImage updates itself; `.deb` and `.rpm` installs show the setting as
unavailable and update through the package manager.

## Resource budget (enforced, not aspirational)

- Hidden island: 0% CPU. Cortex wakes on events, when a card or a session is
  due to expire, and for the GitHub and update checks the user turned on.
- Working set under ~120 MB on all three platforms (about 45 MB measured on
  macOS). A downloaded update waiting for its restart is held in memory, which
  on Linux adds the size of the AppImage until the restart (#72).
- `cortex-hook`: exits < 50 ms when the app is not running.

## Limits

Whatever a local process sends is bounded. Beyond a limit, the agent carries
on as if Cortex were not running: nothing blocks and nothing piles up.

| What | Limit | Beyond it |
|---|---|---|
| Connections served at once | 64 | the new one is closed unread |
| One event | 1 MB | it is dropped; a line far beyond it closes the connection |
| Sessions on the board | 64 | a new session gets no pill (its cards still show); known ones still update |
| Cards waiting | 32 | the agent asks in its own terminal |
