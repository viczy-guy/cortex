# ADR 0002 — No server, no account, no telemetry

**Status:** accepted · 2026-11-15

## Context

Cortex watches coding agents and approves what they may run. That data (prompts,
file paths, commands) is among the most sensitive a developer produces. The
project may also become a commercial product later.

## Decision

Cortex is a local, single-user desktop app. There is no Cortex backend, no
account and no telemetry. Secrets go to the OS vault. Network traffic happens
only toward services the user explicitly configures (e.g. GitHub). Kubernetes
and any multi-tenant infrastructure are explicitly out of scope.

## Consequences

- Zero infrastructure cost and zero custody of user data (strong sales argument,
  trivially LGPD/GDPR-friendly).
- "Enterprise" ideas that require a server (central dashboards, shared policies)
  become a separate opt-in product decided later — the open-core path stays open.
- Multi-user adoption = many independent installs, like any desktop tool.
