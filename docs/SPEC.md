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
  one finished, **red** when one failed. With GitHub on (#131), **blue** when
  someone asks for the user's review, and also when an own pull request is
  approved (#135, blue since #141; not a draft; it stands while approved,
  whatever its checks do), red also when an own pull request or an action the user started
  failed, or an Atlantis plan destroys something (see [GitHub](#github)), and
  **turquoise** (Atlantis's, #173) when an own Atlantis plan is ready and
  only adds or changes, or is applied. The most urgent shows: red, amber,
  blue, turquoise, green. Amber from a card lasts until
  the card is answered or expires. Every other notice lasts until its cause
  goes (the agent moves on, the checks pass, the review is given) or the user
  clicks; green and red stay even after the session is gone. A click on the
  island's bar (its buttons included) clears them all, a click on a session's
  card clears that session. Hovering opens the island in the color and clears
  nothing (#126). When the color rises, the mascot jumps twice, then stays
  still (not at all with reduced motion).
- The lively mascot (#145, on by default; Settings → Island turns it off):
  its eyes follow the mouse anywhere, on any screen (the pixel penguin's a
  pixel at a time), and eyebrows go with them (the mustache's never down,
  onto the glasses; only across while a mood lifts them); it blinks every
  2.5 to 6 s, looks around now and then while the mouse rests, and naps
  after 10 minutes with no agent working, no notice standing and no mouse
  movement (eyes closed, a still "z"). The
  mouse coming near the island, an agent, a notice or a click wakes it. A
  click on the island (not on a button) squashes it with a smile; three
  within 1.5 s make it dizzy for 2 s. When the color rises it jumps with wide
  eyes, smiling for green and turquoise and shaking for red. Only while the mascot is lively,
  Cortex reads the cursor ten times a second while it moves and a few times a
  second once it rests (on macOS straight from Core Graphics, never waking
  the main thread); it tells the island only when the direction changes, one
  of 5 × 3 (`cortex_core::gaze`), measured from the mascot, whose place the
  island reports when it draws or resizes. Eyes and lids switch without
  easing: on a 30 × 18 point face an ease does not show, and each frame is a
  repaint. On macOS a locked screen stops it all, since a repaint of a locked
  island keeps WebKit busy until unlock (#51). Reduce motion removes the
  jumps, shakes and dizziness; the eyes still follow. Where
  the cursor cannot be read (Wayland), the eyes look ahead.
- Each notice has a short phrase and where it happened (#131): "The plan
  destroys 3 resources" (or "replaces"), "The plan failed", "The apply
  failed", "A PR failed!", "An action failed!", "An agent failed" (red); "An
  agent needs you" (amber); "A PR needs your review" and "Your PR #128 (Short
  title) was approved!" (blue); "Plan ready: 10 to add, 2 to change",
  "Applied: 10 added" (turquoise); "An agent finished" (green); beside it the
  repository and number (the repository alone when the phrase names the pull
  request), the workflow and branch, or the agent and its folder. When a new one comes (an agent's state change that has a phrase, or
  a GitHub notice not there before), the folded island widens with its phrase
  and a "+N" for the others standing for 5 seconds, counted from when it is
  drawn, then folds back in the color; beside the camera the phrase takes a
  row under it. Opening the island (its list shows the notice), the notice
  going, or a more urgent color ends the phrase early. A notice less urgent
  than the island's color, or than the phrase showing, does not speak.
  Hovering lists every standing notice, the most urgent first (not over a
  card); a click on a row clears that one, and a GitHub one also opens on
  GitHub. With two or more, "Clear all" on top of the list clears every one,
  as a click on the island does. An Atlantis notice (#161) carries its plan:
  its row shows the counts that are not zero (`+` add, `~` change, `±`
  replace, `−` destroy), and a click clears it and opens its pull request's
  card in the Atlantis tab (#173), with the plan's summary inside, instead
  of GitHub. The summary: one sentence of what the plan does (the most dangerous
  first, the commonest AWS types by name, "and N more", where or across how
  many projects), the alerts (data at risk: an RDS database, an S3 bucket, a
  DynamoDB table, a KMS key, a secret, a user pool, a DNS zone… destroyed or
  replaced; production; drift, "the apply undoes it"; a long plan whose list
  is partial), the changes grouped by project and type, the most dangerous
  project first, "N projects without changes". A create-only
  plan on a review request rides on its review notice. Each account's first GitHub answer (after
  launch, after turning GitHub on, or once the account is shown again) speaks
  nothing: what was there is not news; a notice that went and comes back
  is. In the session cards the "working" dot is gray, as on the island: blue
  is the review's.
- Atlantis, live (#173). While plans or applies run on the user's pull
  requests or on those asking for their review, the island shows Atlantis's
  sign beside the pull request count: a round icon of our own (waves on a
  disc; Atlantis's logo is the Linux Foundation's trademark), a ring turning
  around it, and how many run now (a plan waiting on another pull request's
  lock does not count). A click on it, or on an Atlantis notice, opens the
  Atlantis tab, and the island stays open until a click outside it (on
  macOS Rust reports that click, as over a waiting card). With Atlantis to
  show, the list has two tabs, Notices and Atlantis; it starts on Notices
  while any stands. The Atlantis tab has one card per pull request (runs
  going first, then locked plans however long they wait, then the rest that
  moved in the last hour, at most 8 for all accounts; a merged one only for its apply, for an hour; the one a notice
  opened even when older): its number and title, whose it is, one tag per
  account its projects are in (production in red), and five steps, Prepare
  → Plan → Approval → Apply → Merge, the one now turning, with its clock
  ("2 ready", "+3 ~1", "your review", "your turn", "1 of 3", "by #331").
  One card at a time is open (the first, or the one picked): what runs now
  and for how long ("Preparing Atlantis" while its pre-workflow hooks run,
  "Planning `data/rds` 0:43") with its live log, or what it waits for
  ("Approved. Now comment `atlantis apply`", "Locked by #331"); the plan's
  summary once planned; the projects (running, failed, queued, done with
  their counts, without changes), each with its log; Open PR and Full plan.
  The clocks tick once a second, only while the tab shows them. Nothing
  turns while nobody is looking (#51), and with reduced motion the rings
  stay still.
- While you were away (#157): nobody is looking while the screen is locked
  (macOS), after ten minutes without mouse or keyboard input (macOS and
  Windows), or while the computer sleeps, seen as a jump of the wall clock
  between two looks two seconds apart; coming back from a sleep then waits
  up to a minute for a good GitHub answer asked for after the wake (asked for
  at once). Each look asks the system for the lock and the idle time
  (`cortex_core::away`); the island hears only of a change, and asks at
  launch. Meanwhile nothing rings or speaks, the folded island
  is not redrawn (a locked repaint costs CPU, #51) and the notices that come
  are kept; a card still opens the island, silently. Coming back with
  something kept, the folded island turns light pink with "While you were
  away", a count per color, **See all** (the list of notices; the pink goes)
  and **Dismiss** (everything seen, as a click on the island), and plays the
  soft "Suave" once. A waiting card comes first (its sound rings once on
  coming back), the summary after it. A click on the pink island is See all.
  A notice that comes after is a normal one, phrase and sound, and stays out
  of the summary; leaving again before a click adds to it; a notice clicked
  away or gone leaves it, and each counts in its color of now. Cortex
  closed: the GitHub notice keys of the last check are kept in
  `github-notices.json` in the app data directory (not while a shown account
  has no answer yet), and the first good answer of each account after launch
  (not after turning GitHub on or showing an account) names the new ones,
  which make the summary once the opening screen goes. Dismiss leaves the
  island folded until the pointer comes back. On Linux only a sleep and a closed Cortex count.
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
(green), waiting for the user or a new card (amber), or failed (red); and a
new GitHub notice at least as urgent as the island's color (the rule for its
phrase), in its color, blue being the "Toque" (two notes up, softer than the
alert: a request, not an emergency), and turquoise green's "Alerta" until
the maintainer picks Atlantis's own (#173). If several come together, the most urgent
plays; a lower one arriving while a higher one plays is not played. Coming
back to a pink summary plays "Suave" once (two low notes up), under any other
sound. Sounds are on by default and can be muted or sampled in Settings →
Notices & sounds.

## Opening screen

On manual launch, a small centered window shows a growing colored dot,
"Welcome", then the selected mascot and "Let's get to work?" with the app
version at the bottom. It closes after about 6 seconds. A click skips it
(it never takes the keyboard). With reduced motion, it shows the final
frame without animation for 3 seconds. Launches at login (`--autostart`)
skip it. Rust closes the window after 10 seconds if it is still open.

The island waits for this screen (and for "Update complete") and appears
when it closes; it never stays hidden more than 12 seconds (#152). On a new
install (no saved version and no "open at login" answer), Settings then
opens on the Overview with **Get started**: **Connect Claude Code** (the
reviewed installer, on Agents) and **Connect GitHub** (optional). Each line
goes once it is done; later launches do not open Settings.

The same window shows the updates (#127):

- **Updating…**: a click on **Update and restart** (island or Settings)
  opens it with the mascot, a moving bar (the package is already downloaded
  and checked: there is no percentage to show) and "0.3.1 → 0.3.2". The
  install starts after about a second, so the screen is seen.
- **Update complete, let's get to work?**: the new copy shows it at its
  first launch, with the mascot green and the version at the bottom, even at
  login or after a recovery restart (an update restart keeps the old
  arguments). It closes after about 4 seconds, or on a click. Each launch
  saves its version in the preferences (`last_version`); another one there
  means an update, however it was installed. 0.3.0 and 0.3.1 saved none: a
  saved "open at login" answer then tells an update from a new install.
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

The gear in the island opens a separate, opaque Settings window with a
sidebar of pages (#151); the arrow keys move through it. A new Settings
window opens on the Overview.

- **Overview** has one line per subject (Claude Code, Always rules, GitHub,
  Island, Sounds, Updates, Open at login) with its state; a click opens its
  page.
- **Agents**: **Claude Code** shows hook status and offers reviewed
  installation or removal, with the exact diff and a dated backup before
  writing. **Create an agent** is a disabled card that says it comes in a new
  version. **Always rules** lists local rules and allows removing one or all;
  removing all requires a second click within four seconds.
- **Island** chooses the mascot from a gallery of twelve, or none, turns the
  lively mascot on or off, and sets the island's screen and position.
- **Notices & sounds** explains the four colors, each with its sound to play,
  and turns the sounds on or off.
- **GitHub** turns the pull request pill on or off and manages its accounts
  (see [GitHub](#github)).
- **General** turns launch at login on or off (see below), and the update
  check, with **Update and restart** once a verified update is ready. It
  also picks the language (see [Language](#language)).
- **About** shows the app version, a "What's new" link to the notes of that
  version, its local-only policy, and Quit Cortex.

On macOS, Cortex also has a menu-bar item with **Settings…** and **Quit
Cortex** (**Ajustes…** and **Encerrar o Cortex** in Portuguese). It is a native menu, so it still quits when the island's web view
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

## Language

Every text Cortex shows is in English and in Portuguese (#154). **System**,
the default, follows the first of the system's preferred languages:
Portuguese when it is Portuguese, English otherwise (on Linux, `LC_ALL`,
`LC_MESSAGES` or `LANG`). Settings → General → Language picks **English** or
**Português** for good, or goes back to **System**. Every window changes at
once: the island draws again in place, Settings opens again on the page it
was on, and the menu-bar item and the Settings title follow. The opening
screen reads the language before it draws. The Windows setup (`.exe`) is in
English or Portuguese, following the system; the `.msi` is in English. What
GitHub, Atlantis and the agents write (titles, plans, commands, folders)
stays as they wrote it.

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
Settings. Cortex checks shown accounts about every 15 seconds while on (15
seconds after the last check ended, or when Refresh now is clicked). Once a
minute while GitHub keeps failing, and on macOS while the screen is locked;
unlocking is seen within 15 seconds. Off means no polling or timers.

The island pill totals requested reviews, failing own PRs, and open PRs across
shown accounts. Its expanded GitHub card groups requested reviews and own pull
requests by account and shows checks, draft and review status. Each account
keeps its last good data after a fetch error. GitHub does not change when the
island folds.

Settings can run gh's browser sign-in, show its one-time code, and restore the
previous active gh account after adding an account. If gh is missing, Settings
offers to install it with Homebrew on macOS or winget on Windows when available,
or links to the GitHub CLI site. Cortex validates each account token and passes
it only in the environment of that account's `gh api` children, without
storing or logging it.

Each check of an account runs up to three requests, all reads (#131): the
summary query above; GitHub's unread notices of the last day, of which only
"… workflow run failed for …" (an action the user started) is kept, one per
workflow and branch, read no sooner than GitHub's `X-Poll-Interval` (60 s
usually, at most an hour honored) after the last read, Refresh now included;
and, for
up to 2 pull requests with an Atlantis run past its plan whose `updatedAt`
changed since their comments were last read (the least recently tried first,
per account), their last 8 comments (#161).

Atlantis (#161, `cortex_core::atlantis`): the summary query also reads the
statuses of each pull request's last commit, with their links, and the
user's last 5 merged pull requests. `atlantis/plan` and `atlantis/apply`
tell the stage (planning, planned, plan failed, applying, applied, apply
failed; an apply counts when it came after the latest plan), each
`atlantis/plan: <project>` its counts ("Plan: 2 to add, 1 to change, 0 to
destroy.", or "No changes."). While a run goes (#173), a pending
`atlantis/pre_workflow_hook: …` means Atlantis prepares, else a pending
project status is the project running, since its time; a running plan lists
only the projects of this run, an apply each planned project (queued,
running, done, failed, not applied, or skipped without changes). A status's
link is kept only when the account that wrote the `atlantis/plan` status,
one whose name has "atlantis" in it, wrote it too, and only as an https link
to a host's `/jobs/` page, at most 512 characters. The
comments add the detail: the latest "Ran Plan for…" and its "Continued…"
parts (a quote does not count), project by project ("### 1. project: `a`
dir: `b`…"), each resource line (`# aws_x.y will be created`, `updated
in-place`, `must be replaced`, `will be destroyed`; reads, moves and imports
are left out), behind Terragrunt's log prefix or not, drift ("Objects have
changed outside of Terraform"), the first "Error: …" of a "**Plan Error**" or
"**Apply Error**", a "**Plan Failed**:" or "**Apply Failed**:" reason, and a
lock ("locked by … #331"). The comments' counts replace the statuses' only
when they add up to them (replacements told apart); otherwise the statuses'
stand and the list is partial (a plan longer than the comments read, up to 60
groups). A detail is only shown with the plan run it was read for; while
planning, none. A plan failed only on locks waits, without a notice. The
environment of a project is the folder right before the region
(`…/production/sa-east-1/…`; a generated project name joins folders with
`_`), else, without a region, a production folder anywhere; production is
`production`, `prod`, `prd`, `infraestructure` or `network`, as the
maintainer set them. A failed notices request keeps the last failed actions;
a failed comments request is tried again after the others; when the comments
in sight hold no plan, what was known for the same run stands.

From all this the account's summary carries its notices: a review requested
(with its plan when that destroys nothing), an own pull request approved (not
a draft; with failing checks, the failing notice stands beside it), one with
failing checks (not when Atlantis failed: its notice says why), a plan that
destroys or replaces something (own or review), an own plan ready that only
adds or changes, a failed plan, an own apply done (for an hour, merged or
not) or failed, a failed action (opening the repository's Actions page, since
GitHub's notice names no run; one on the branch of an own pull request is left
to that pull request's notice). An Atlantis notice is new with each run. The
summary also carries the Atlantis tab's pull requests (#173).

Cortex opens GitHub links from the current summaries (Atlantis's live logs
included, #173), the fixed GitHub device
login page, and the GitHub CLI site through the OS with the URL as one argument
and no shell.

## Updates

Opt-in (ADR 0004). On first launch the island asks once, "Check for updates
automatically?"; until the answer is Yes, Cortex makes no update request. The
answer can be changed in Settings → General.

While on, Cortex fetches the release manifest (`latest.json` of the latest
published GitHub release) at launch and every minute while Cortex is being
tested (back to every 6 hours afterwards, #158; after a package that failed
to download or verify, 6 hours), and sends nothing but that
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
  due to expire, for the GitHub and update checks the user turned on, and
  every 2 seconds to read the clock, the lock and the idle time, to know
  whether anybody is looking (#157).
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
