# Cortex — behavior spec (v1)

Short, normative description of how the app behaves. When code and spec diverge,
one of them is wrong — fix that one.

## The island

- A transparent, always-on-top, undecorated window hugging the top-center of the
  screen. It never steals focus: clicking a button must not deactivate the app
  the user is typing in (macOS: non-activating panel).
- States: `hidden` (nothing to show — **0% CPU**, no timers, no animation frames)
  → `compact` (a slim bar with one pill per active session) → `expanded`
  (session detail, approval/question cards) on hover or click.
- Empty areas of the window are click-through.
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
- The decision is bound to the exact request id; anything else (timeout, stale
  id, duplicate) resolves to "no decision" and the terminal takes over.
- The card disappears the moment its connection closes (hook timeout, or the
  user answered in the terminal). The wait is 50 s: while the hook waits, the
  terminal shows no prompt of its own, so the island must not hold it long.
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

## Resource budget (enforced, not aspirational)

- Hidden island: 0% CPU (event-driven wakeups only).
- Working set under ~120 MB on all three platforms.
- `cortex-hook`: exits < 50 ms when the app is not running.
