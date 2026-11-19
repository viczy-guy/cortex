# Cortex — behavior spec (v1)

Short, normative description of how the app behaves. When code and spec diverge,
one of them is wrong — fix that one.

## The island

- A transparent, always-on-top, undecorated window hugging the top-center of the
  screen. It never steals focus: clicking a button must not deactivate the app
  the user is typing in (macOS: non-activating panel).
- States: `hidden` (no card waiting — **0% CPU**, no timers, no animation
  frames) → `compact` (a slim bar with one pill per active session, on hover)
  → `expanded` (session detail, approval/question cards) on click, or by
  itself when a card needs the user.
- Hidden means folded into a small handle at the top-center, even while
  sessions run. Hovering the handle (or clicking it) brings the bar back, with
  the gear to Settings; it folds again shortly after the pointer leaves, and
  once the last card is answered. The island never hides while
  the hooks are missing — that is when "Connect Claude Code" must be seen.
- A click anywhere outside the island folds it, even over a waiting card: the
  handle turns amber until every folded card is answered or expires, and
  hovering brings the cards back. A new card opens the island again. (macOS: a
  global mouse-down monitor, which needs no permission and only wakes the UI
  while a card waits; Windows / Linux: the window's blur, so only after the
  island had focus.)
- The window is always exactly as large as what it draws, so no invisible area
  eats clicks. The UI cannot move or resize windows itself: it asks Rust for a
  size, which is clamped and kept top-centered.
- Theming: all colors/spacing come from CSS tokens under a `data-theme`
  attribute. Personas (e.g. a future mascot) are additive themes, never forks of
  the UI.

## Sessions

One pill per (`cortex_agent`, `session_id`). State machine:

| Event | State |
|---|---|
| `SessionStart` | `idle` |
| `UserPromptSubmit` | `thinking` (prompt in ticker) |
| `PreToolUse` | `working` (tool label in ticker) — or `waiting_approval` when a decision is requested |
| `PostToolUse` | `working`; file edits record name and +N/−M |
| `Notification` | `waiting_input` when the agent needs the user |
| `Stop` | `finished` (5 s), then pill idles or is removed |
| `SessionEnd` | pill removed (declared pills reset to `idle`) |

Sessions stale for 30 min with no events are garbage-collected.

## Approvals

- A `PermissionRequest` shows a card: tool, detail (command / file path),
  **Allow / Deny / Always** and "Answer in terminal".
- An `Edit` card shows the exact change and a `Write` card the whole new
  content, as a line diff (at most 300 lines; the rest is counted, and the
  terminal shows it all). Redraws keep the scroll position.
- The decision is bound to the exact request id; anything else (timeout, stale
  id, duplicate) resolves to "no decision" and the terminal takes over.
- The card disappears the moment its connection closes (hook timeout, or the
  user answered in the terminal). The wait is 110 s (hook timeout 120 s), long enough to read a
  multi-question card; "Answer in terminal" hands it back sooner.
- "Always" stores a rule applied by Cortex, never written into the agent's own
  settings. The card states what the rule covers before the click.
  - Shell commands generalize to their program: Always on `cargo test` allows
    every `cargo …`. Only a single plain command can create or match a rule —
    a line with chaining, pipes, redirection, substitution (`; & | < > $ ( )`
    `{ }`, backticks, backslash, newline), a `VAR=value` prefix or a command
    runner (`sudo`, `sh`, `env`, `xargs`, …) always shows a card and offers no
    Always.
  - Every other tool must match its detail exactly.
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
- **About** shows the app version, its local-only policy, and Quit Cortex.

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

## Resource budget (enforced, not aspirational)

- Hidden island: 0% CPU (event-driven wakeups only).
- Working set under ~120 MB on all three platforms.
- `cortex-hook`: exits < 50 ms when the app is not running.
