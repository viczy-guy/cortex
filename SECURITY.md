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
| Hook socket / named pipe | Restricted to your OS user (0600 on Unix, per-user pipe on Windows). `CORTEX_SOCKET` lets you relocate it, never widen it. |
| Hook payloads | Treated as untrusted input: schema-validated in Rust before anything reaches the UI; unknown fields ignored; malformed events dropped, never crash the app. |
| Approval decisions | Bound to the exact request the agent is waiting on (request id + session). A late or replayed decision approves nothing. No automatic approval of anything. |
| WebView | Strict CSP, no remote content, the UI never interpolates external data as HTML. Tauri capabilities expose the minimum set of commands. |
| Secrets | OS vault only (Keychain / Credential Manager / Secret Service). Never on disk, never in logs, never in git. |
| Network | None, except services you explicitly configure. No telemetry, no account. |
| Supply chain | Minimal, established dependencies; lockfiles committed; `cargo audit`, `cargo deny` and `npm audit` run in CI and fail the build on known vulnerabilities. |
| Updates | Release artifacts are signed with the project key and verified before install; SHA-256 checksums published with every release. |

## What Cortex never does

- Approve a permission without an explicit click.
- Overwrite your agent settings: the hook installer makes a dated backup, merges,
  shows the diff and writes only after you confirm.
- Phone home.
