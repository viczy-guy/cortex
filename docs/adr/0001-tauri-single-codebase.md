# ADR 0001 — Tauri 2, single codebase for the three platforms

**Status:** accepted · 2026-11-15

## Context

Cortex must run on macOS, Windows and Linux, stay light (low RAM, 0% CPU when
hidden) and be maintained by a very small team. The reference project, coucou,
ships a native Swift app on macOS plus a separate Tauri app for Windows/Linux —
two codebases, every feature built twice.

## Decision

One Tauri 2 codebase for all three platforms. Rust core (all logic), system
WebView for the UI (strict TypeScript, minimal npm deps). No Electron (bundled
Chromium breaks the resource budget); no pure-Rust UI toolkits (immature, slower
UI iteration, larger WASM payloads).

## Consequences

- One implementation, one test suite, one release pipeline.
- The macOS island needs a non-activating panel (NSPanel behavior) that plain
  Tauri windows don't provide; validated by the Phase 0 spike before building on it.
- UI is limited to what the system WebViews support — acceptable for an island.

## Revisit if

The macOS spike fails to deliver a quality island — fallback is a minimal native
window layer on macOS only, keeping core, hook and UI shared.
