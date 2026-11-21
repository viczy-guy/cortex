#!/usr/bin/env bash
# Smoke test of an installed Cortex, for machines nobody is looking at (CI):
# start the app, then ask it for a decision through the relay it ships.
#
#   scripts/smoke-test.sh <path to the installed app binary>
#
# The relay is silent by design, so the proof that app and relay talk is an
# Always rule seeded before launch: a matching PermissionRequest comes back
# "allow" at once, and only a running Cortex that loaded the rule answers.
# Passing means: the app starts and stays up, it copied its bundled relay to
# the data directory (the path the hooks use), and the socket or pipe works.
#
# It writes rules.json in the user's data directory: run it on a throwaway
# machine or with HOME / XDG_DATA_HOME / LOCALAPPDATA pointing somewhere empty.

set -euo pipefail

app="${1:?usage: smoke-test.sh <path to the installed app binary>}"
[ -f "$app" ] || { echo "::error::no app binary at $app"; exit 1; }

case "$(uname -s)" in
  Darwin) data="$HOME/Library/Application Support/Cortex"; exe="" ;;
  Linux) data="${XDG_DATA_HOME:-$HOME/.local/share}/cortex"; exe="" ;;
  *) data="$LOCALAPPDATA/Cortex"; exe=".exe" ;;
esac
relay="$data/bin/cortex-hook$exe"

session="smoke-$$"
start="{\"hook_event_name\":\"SessionStart\",\"session_id\":\"$session\"}"
request="{\"hook_event_name\":\"PermissionRequest\",\"session_id\":\"$session\",\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"cargo test\"}}"

mkdir -p "$data"
rm -f "$relay"
printf '{"version":1,"rules":[{"tool_name":"Bash","pattern":"cargo"}]}\n' > "$data/rules.json"

"$app" &
pid=$!
trap 'kill "$pid" 2>/dev/null || true' EXIT

answer=""
for _ in $(seq 30); do
  kill -0 "$pid" 2>/dev/null || { echo "::error::the app exited during startup"; exit 1; }
  if [ -x "$relay" ]; then
    printf '%s\n' "$start" | "$relay" || true
    asked=$SECONDS
    answer="$(printf '%s\n' "$request" | "$relay" || true)"
    # No answer after a long wait: the app showed a card instead of applying
    # the rule. Asking again would only wait again.
    [ -z "$answer" ] && [ $((SECONDS - asked)) -lt 5 ] || break
  fi
  sleep 1
done

[ -x "$relay" ] || { echo "::error::the app did not copy its relay to $relay"; exit 1; }
case "$answer" in
  *'"behavior":"allow"'*) echo "ok  the relay reached the app: $answer" ;;
  *) echo "::error::no decision from the app through the relay (got: '$answer')"; exit 1 ;;
esac

sleep 5
kill -0 "$pid" 2>/dev/null || { echo "::error::the app did not stay up"; exit 1; }
echo "ok  the app is still running after 5 s"
