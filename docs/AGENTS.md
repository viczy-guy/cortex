# Cortex — agent integration protocol

Any tool that can write to a Unix domain socket (macOS, Linux) or a named pipe
(Windows) can send events to Cortex and get its own pill in the island.

## Endpoints

| OS | Endpoint |
|---|---|
| macOS | `~/Library/Application Support/Cortex/cortex.sock` |
| Linux | `$XDG_RUNTIME_DIR/cortex.sock` |
| Windows | `\\.\pipe\cortex-<username>` |

`CORTEX_SOCKET` overrides the path (useful for SSH tunnels from remote machines).
The endpoint only accepts connections from your own OS user.

## Wire format

Newline-terminated JSON, one event per line:

```json
{"hook_event_name": "UserPromptSubmit", "session_id": "abc", "cortex_agent": "my-tool", "prompt": "Fix the tests"}
```

- `hook_event_name` (required): the Claude Code hook event names are the
  canonical vocabulary (`SessionStart`, `UserPromptSubmit`, `PreToolUse`,
  `PostToolUse`, `Notification`, `Stop`, `SessionEnd`, …).
- `session_id` (required): any stable string for the session.
- `cortex_agent` (optional): `^[a-z0-9-]{1,24}$`. Routes the event to a pill
  with that name; absent or invalid falls back to `claude-code`.
- Unknown fields are ignored. Malformed lines are dropped. An event never
  crashes or blocks Cortex.

## Blocking events (approvals)

For `PreToolUse`, the connection may stay open: Cortex replies on the same
connection with a single JSON line once the user decides —

```json
{"decision": "allow"}        // or "deny", or "ask" (fall back to the terminal)
```

If Cortex is not running, or the user does not decide in time, the relay exits
with no output and the agent asks in its own terminal. **Cortex never blocks an
agent.**

## The `cortex-hook` relay

Agents normally don't speak to the socket directly — they call the `cortex-hook`
binary, which reads the event JSON on stdin, injects `--agent <name>` as
`cortex_agent`, forwards it, and relays the decision for blocking events:

```json
{ "hooks": { "UserPromptSubmit": [ { "hooks": [
  { "type": "command", "command": "/path/to/cortex-hook --agent my-tool" }
] } ] } }
```
