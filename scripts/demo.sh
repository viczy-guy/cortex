#!/usr/bin/env bash
# Simulate a Claude Code session against a running Cortex, without touching
# any real agent or settings file. Useful to try the island and to develop.
#
#   scripts/demo.sh            # a session, then a permission request
#   scripts/demo.sh question   # a session, then a question
#
# The last step blocks until you decide in the island (or 50 s pass) and then
# prints what the relay would hand back to the agent.

set -euo pipefail

cd "$(dirname "$0")/.."
HOOK="${CORTEX_HOOK:-target/debug/cortex-hook}"
[ -x "$HOOK" ] || { echo "Build the relay first: cargo build -p cortex-hook" >&2; exit 1; }

SESSION="demo-$$"
send() { printf '%s\n' "$1" | "$HOOK"; }

send "{\"hook_event_name\":\"SessionStart\",\"session_id\":\"$SESSION\"}"
send "{\"hook_event_name\":\"UserPromptSubmit\",\"session_id\":\"$SESSION\",\"prompt\":\"Refactor the payment module and add tests\"}"
sleep 1
send "{\"hook_event_name\":\"PreToolUse\",\"session_id\":\"$SESSION\",\"tool_name\":\"Edit\",\"tool_input\":{\"file_path\":\"/demo/src/payment.rs\"}}"
send "{\"hook_event_name\":\"PostToolUse\",\"session_id\":\"$SESSION\",\"tool_name\":\"Edit\",\"tool_input\":{\"file_path\":\"/demo/src/payment.rs\",\"old_string\":\"fn pay() {\\n  todo!()\\n}\",\"new_string\":\"fn pay(amount: u64) -> Result<Receipt, Error> {\\n  validate(amount)?;\\n  charge(amount)\\n}\"}}"
sleep 1

if [ "${1:-}" = "question" ]; then
  echo "A question is waiting in the island…"
  send "{\"hook_event_name\":\"PreToolUse\",\"session_id\":\"$SESSION\",\"tool_name\":\"AskUserQuestion\",\"tool_input\":{\"questions\":[{\"question\":\"Which test framework should I use?\",\"header\":\"Tests\",\"multiSelect\":false,\"options\":[{\"label\":\"Built-in\",\"description\":\"cargo test\"},{\"label\":\"Nextest\",\"description\":\"Faster runner\"}]}]}}"
else
  echo "A permission request is waiting in the island…"
  send "{\"hook_event_name\":\"PermissionRequest\",\"session_id\":\"$SESSION\",\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"cargo test --workspace\"}}"
fi
echo "(nothing printed above = no decision: the agent would ask in its terminal)"

send "{\"hook_event_name\":\"Stop\",\"session_id\":\"$SESSION\"}"
