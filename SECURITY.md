# Security

## Reporting a vulnerability

Please report vulnerabilities privately through
[GitHub Security Advisories](https://github.com/viczy-guy/cortex/security/advisories/new).
Do not open a public issue for a security problem. You will get an answer within
7 days.

## Threat model (summary)

Cortex approves permissions for agents that execute code on your machine — it is
a gatekeeper, and is designed as one:

| Surface | Control |
|---|---|
| Hook socket / named pipe | Restricted to your OS user (0600 on Unix; on Windows a per-user pipe that only its owner can open). `CORTEX_SOCKET` lets you relocate it, never widen it. Programs running as you are trusted on this channel, as they already are with your agent's settings: one can claim to be any session and ask for a card, but cannot answer one. Lines are size-capped before they are read into memory, and connections, sessions and waiting cards have a ceiling. |
| Hook payloads | Treated as untrusted input: schema-validated in Rust before anything reaches the UI; unknown fields ignored; malformed events dropped, never crash the app. |
| Approval decisions | Bound to the exact request the agent is waiting on (an id Cortex assigns to its connection). A late or replayed decision approves nothing. Nothing is approved automatically except by an "Always" rule you created with a click; a rule covers one tool and one exact target (for shell commands, one program) and never a command that chains, pipes, redirects or substitutes. Rules are listed and removable in Settings. |
| WebView | Strict CSP, no remote content, the UI never interpolates external data as HTML. Each window may call only the app commands it uses (Tauri capabilities per window): the island cannot quit, uninstall hooks or change rules, and Settings cannot answer cards. |
| Secrets | OS vault only (Keychain / Credential Manager / Secret Service). Never on disk, never in logs, never in git. |
| Network | None, except services you explicitly configure, and the update check if you said yes to it (one request for the release manifest, at launch and every 6 hours). No telemetry, no account. |
| Supply chain | Minimal, established dependencies; lockfiles committed; `cargo audit`, `cargo deny` and `npm audit` run in CI and fail the build on known vulnerabilities. |
| Updates | Opt-in. Update packages are signed with the project key (bound to their version) and verified before install; the key never reaches the build machines and every signature is checked against the app's key before release. Installed only on your click, never over a waiting card. SHA-256 checksums published with every release. |

## What Cortex never does

- Approve a permission without an explicit click — yours on the card, or the
  one that created the "Always" rule it matches.
- Overwrite your agent settings: the hook installer makes a dated backup, merges,
  shows the diff and writes only after you confirm.
- Phone home.
