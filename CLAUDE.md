# Cortex — notes for Claude

The project rules, layout and required checks are in @AGENTS.md — they apply
to every AI agent working here, Claude included.

## Picking the work back up

- **What is done and what is next:** `ROADMAP.md` (checkboxes), then `git log`.
- **Who is doing what:** the project board,
  https://github.com/users/viczy-guy/projects/1/views/11 (the "Fases" view;
  issues and the onboarding guide, #36, are in Portuguese).
- **How it must behave:** `docs/SPEC.md`. **Why it is built this way:** `docs/adr/`.
- **Try it:** `npm run tauri dev`, then `scripts/demo.sh` in another terminal.
  `CORTEX_DEV_OPEN_SETTINGS=1` (debug builds) opens Settings at launch.

## How we work

- Claude specifies, reviews and decides; well-scoped implementation tasks are
  delegated to the Codex CLI from a written spec. Security-sensitive code
  (socket, approvals, anything that writes user files), architecture and
  visual debugging stay with Claude.
- Delegated runs log to `.codex-runs/` (git-ignored) so the user can follow
  them with `tail -f`. Codex never commits; Claude re-runs every check outside
  the Codex sandbox and reviews the diff before committing.

## Sharing the repository

Other people work here too, and `main` is protected: every change reaches it
through a pull request with green CI. A direct push only "works" because the
user is an admin — never rely on that.

- **Before starting**, find the issue on the board and check that nobody is
  assigned or has it "Em andamento". Claim it: assign the user and move it to
  "Em andamento". No issue yet? Create one first.
- **Work on a branch linked to the issue**, from an up-to-date `main`:
  `gh issue develop <N> --checkout`. Never commit on `main`.
- **Open a draft PR after the first commit** (`gh pr create --draft`, body
  `Closes #<N>`), so the others see early which files are moving. Two pieces
  of work in the same `área:*` label at once: coordinate on the issue first.
- **Before ending a session:** commit and push the branch, and say in the PR
  what is done and what is left. When the work is complete and the checks
  pass locally, mark the PR ready and move the issue to "Em revisão".
- **Merge** (`gh pr merge --squash --delete-branch`) only with green CI and
  the user's OK for that change; during autonomous work, leave the PR ready
  instead. The PR title becomes the commit on `main`, so write it as a
  Conventional Commit.
- A PR that changes what users get adds its line to `CHANGELOG.md` under
  `[Unreleased]`.
  `ROADMAP.md` changes only when a phase or a release closes, which keeps
  conflicts away.
