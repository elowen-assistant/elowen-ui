# elowen-ui

Client-side Leptos frontend for Elowen's chat-first workspace.

## Current Responsibilities

- provide the authenticated web UI for threads, messages, jobs, approvals, and details
- keep chat as the primary surface, with jobs and context available through Material-style navigation
- render a Material-inspired responsive shell with a pinned composer and scroll-contained message pane
- preserve selected thread, selected job, nav mode, details state, composer text, and transcript scroll behavior across background updates
- subscribe to authenticated server-sent events for thread, job, and device changes
- retain slower polling as a fallback while realtime behavior is hardened
- expose stable `data-testid` hooks for the planned Playwright browser automation slice

## Runtime Notes

The UI is still client-side rendered and built with Trunk. The VPS deployment serves a prebuilt GHCR image rather than compiling on the server.

SSR is intentionally deferred. The current product issue is long-running app-state continuity, not initial render quality.

## Verification

Useful local checks:

```bash
cargo fmt --check
cargo test --quiet
cargo clippy --all-targets -- -D warnings
cargo doc --no-deps
```
