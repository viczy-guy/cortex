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

- A `PreToolUse` carrying a pending permission shows a card: tool, detail (command
  / file path), **Allow / Deny / Always** and "Answer in terminal".
- The decision is bound to the exact request id; anything else (timeout, stale
  id, duplicate) resolves to "no decision" and the terminal takes over.
- "Always" stores a local rule (tool + pattern) applied by Cortex, never written
  into the agent's own settings.

## Questions

`AskUserQuestion` payloads render their options as buttons (single or
multi-select, up to 4 questions); the chosen answer is returned on the same
connection. "Answer in terminal" returns no decision.

## Hook installer

Settings → Claude Code → Install hooks: reads `~/.claude/settings.json`, writes a
dated backup next to it, merges Cortex entries (never touching existing ones),
shows the exact diff, and writes only after the user confirms.

## Resource budget (enforced, not aspirational)

- Hidden island: 0% CPU (event-driven wakeups only).
- Working set under ~120 MB on all three platforms.
- `cortex-hook`: exits < 50 ms when the app is not running.
