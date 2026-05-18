## Why

Clankers now has deterministic replay at the engine boundary and at the live controller/agent shell boundary. The remaining high-risk seam is persisted session history and resume: a turn can pass in memory while JSONL/session restore drops user text, assistant tool-call context, tool results, or `_session_id` metadata needed by routed providers.

## What Changes

- Add a credential-free deterministic replay that persists an initial tool turn and resumes it through the real session/controller boundary.
- Assert resumed provider requests preserve restored history, tool-result context, and session metadata.
- Normalize resumed replay artifacts and bind them with a stable BLAKE3 receipt hash.
- Add the persisted-session rail to the existing `deterministic` test-harness profile and dry-run contract.

## Scope

In scope: one focused root integration test, fake provider/tool fixtures, isolated temp session state, deterministic receipt hashing, harness wiring, and OpenSpec archive.

Out of scope: live provider credentials, network access, daemon sockets, broad session-store rewrites, UI replay snapshots, or changing JSONL format beyond fixing discovered resume defects.

## Verification

Run focused deterministic replay tests, harness contract tests, `scripts/test-harness.sh deterministic`, OpenSpec validation, formatting, and `git diff --check`.
