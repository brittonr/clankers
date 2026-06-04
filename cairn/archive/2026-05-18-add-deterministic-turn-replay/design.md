## Context

The current Clankers verification stack includes unit tests, fake-provider E2E tests, TUI snapshots, process-job fixtures, Nix/VM checks, and a canonical harness with run-scoped receipts. It still lacks one cheap rail that proves a complete model→tool→model turn is replayable with stable request, transcript, event, and receipt outputs.

This change intentionally targets the narrow first deterministic backbone: one scripted successful turn with a tool call and one or more negative/correlation cases. It should not require live providers, OAuth credentials, daemon sockets, external services, or VM/KVM.

## Goals / Non-Goals

**Goals:**
- Add deterministic replay fixtures for one complete agent turn.
- Prove replay equivalence across repeated runs in isolated temp state.
- Pin provider request shape and event/transcript normalization.
- Expose the rail through the canonical test harness.

**Non-Goals:**
- Full live-provider determinism.
- Antithesis/ChaosControl VM exploration.
- Replacing existing readiness, E2E, TUI, or VM tests.
- Broad engine refactors beyond seams needed to inject deterministic provider/tool/clock/session inputs.

## Decisions

### 1. Fixture-owned scripted model and tool feedback

**Choice:** Represent deterministic turns as fixtures containing user prompt, session id, scripted provider responses, expected tool calls/results, and expected normalized outputs.

**Rationale:** Fixtures make failures inspectable and prevent hidden dependence on live provider behavior, clocks, ambient config, or filesystem state.

**Alternative:** Only use hand-written Rust tests with inline expected values. Rejected because stable fixture files are easier to review, diff, and reuse across harness modes.

### 2. Normalize volatile fields before hashing

**Choice:** Normalize timestamps, temp paths, process IDs, durations, and any random IDs that are not semantically part of the replay contract before comparing outputs and computing BLAKE3 hashes.

**Rationale:** Deterministic tests should fail on semantic drift, not on unavoidable tempdir or wall-clock differences.

**Alternative:** Mock every volatile source perfectly. Rejected for the first slice because some existing layers may still surface shell-owned paths/times; normalization is a smaller, safer seam.

### 3. Assert request shape explicitly

**Choice:** The fake provider records each completion request and the test asserts session metadata, message ordering, tool schema presence, and continuation content explicitly.

**Rationale:** Past Clankers regressions often came from subtle provider request-shape drift. Snapshotting only final output would miss those failures.

### 4. Harness integration is a profile, not a broad full-gate expansion

**Choice:** Add `deterministic` as a cheap harness profile and list it in `list`/`profiles`; do not automatically expand `full` until the rail is stable.

**Rationale:** This makes the rail discoverable and runnable without increasing default full-gate cost or coupling it to unfinished fixture expansion.

## Risks / Trade-offs

- If the test uses too much runtime shell state, normalization can hide bugs. Mitigation: keep normalization allowlisted and assert raw request shape separately.
- If helpers are too Clankers-app-specific, they will not help `clankers-engine` extraction. Mitigation: keep pure fixture loading/normalization separate from shell execution.
- If only the happy path is covered, replay confidence is limited. Mitigation: include at least one negative/correlation rejection fixture before completion.

## Validation Plan

- `cargo fmt --check`
- focused deterministic replay cargo/nextest test
- `CARGO_TARGET_DIR=target cargo test -p clankers --test test_harness_contract -- --nocapture`
- `openspec validate add-deterministic-turn-replay --strict --json`
- `git diff --check`
