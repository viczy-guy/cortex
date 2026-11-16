# Cortex — notes for Claude

The project rules, layout and required checks are in @AGENTS.md — they apply
to every AI agent working here, Claude included.

## Picking the work back up

- **What is done and what is next:** `ROADMAP.md` (checkboxes), then `git log`.
- **How it must behave:** `docs/SPEC.md`. **Why it is built this way:** `docs/adr/`.
- **Try it:** `npm run tauri dev`, then `scripts/demo.sh` in another terminal.

## How we work

- Claude specifies, reviews and decides; well-scoped implementation tasks are
  delegated to the Codex CLI from a written spec. Security-sensitive code
  (socket, approvals, anything that writes user files), architecture and
  visual debugging stay with Claude.
- Delegated runs log to `.codex-runs/` (git-ignored) so the user can follow
  them with `tail -f`. Codex never commits; Claude re-runs every check outside
  the Codex sandbox and reviews the diff before committing.
- Before ending a session: update `ROADMAP.md`, commit, push.
