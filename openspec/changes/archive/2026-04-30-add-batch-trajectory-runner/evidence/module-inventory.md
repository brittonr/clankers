# Batch Trajectory Runner Module Inventory

## Existing implementation

- There is no dedicated batch trajectory runner yet. Repository searches show existing uses of "batch" are unrelated session-store/worktree helper operations, and searches for `trajectory`/`ShareGPT` do not identify a runner or export format implementation.
- Session export already exists in `crates/clankers-session/src/export.rs` for markdown, text, JSONL, and structured JSON. That module is the best ownership point for ShareGPT/eval trajectory serialization because it already loads both `.jsonl` and `.automerge` session files and normalizes session entries.
- Session CLI export already exists through `src/cli.rs` (`SessionAction::Export`) and `src/commands/session.rs`. Batch trajectory export should reuse those output-directory and `.clankers/exports` conventions rather than inventing a separate destination policy.
- Slash-command export exists in `src/slash_commands/handlers/info.rs` and attach parity tests exist in `src/modes/attach.rs`. Any TUI-facing trajectory export should keep `/export` behavior separate from batch-run execution unless the API surface explicitly extends it.
- Prompt execution entrypoints already exist in headless and interactive modes (`src/modes/print.rs`, `src/modes/json.rs`, `src/modes/inline.rs`, `src/modes/interactive.rs`) and daemon/session routing (`src/modes/daemon/*`, `crates/clankers-controller`). A batch first pass should avoid duplicating these paths and should either call the existing one-shot prompt machinery or define a narrow command adapter around it.
- Concurrency controls already exist for daemon sessions (`src/modes/daemon/config.rs` has maximum concurrent sessions) and subagent/delegation tooling already exercises parallel agent execution. Those are references for bounded concurrency, but a batch runner should have its own deterministic concurrency limit and resumability metadata.

## Proposed ownership boundaries

- `src/cli.rs`: user-facing top-level batch command shape and argument parsing.
- `src/main.rs` and `src/commands/mod.rs`: dispatch to a batch command handler.
- New `src/commands/batch.rs` or equivalent: file input parsing, bounded job orchestration, output path policy, and top-level error reporting.
- New adapter module such as `src/modes/batch.rs` or `src/batch.rs`: reusable batch job types, validation, concurrency/resume state, and safe metadata shaping.
- `crates/clankers-session/src/export.rs`: trajectory/ShareGPT export serialization for completed runs and/or existing sessions.
- `crates/clankers-session/src/entry.rs`: source of normalized message/session metadata; avoid storing backend-specific blobs.
- `crates/clankers-controller` and daemon session actors: reuse only if the first-pass API runs each prompt through daemon sessions. Otherwise document daemon parity as unsupported or deferred.
- `crates/clankers-config/src/settings.rs`: durable defaults only if the first-pass API needs configured concurrency/output settings; otherwise keep CLI flags explicit.
- `README.md` and docs reference pages: document input format, output directory, concurrency/resume behavior, and unsupported first-pass surfaces.

## First-pass scope recommendation

A safe scoped first pass should expose an explicit CLI batch mode that reads a local JSONL/JSON prompt list, runs prompts with bounded concurrency, writes local result/trajectory files under an explicit output path or `.clankers/exports`, and returns structured errors for unsupported remote inputs, unbounded concurrency, nonlocal output paths, or live TUI batch control.

The first pass should not silently add model-callable batch tools, remote dataset fetching, background daemon scheduling, or training uploads. Those cross persistence/security boundaries and should be follow-up OpenSpec work if needed.

## Targeted validation

- CLI/parser and validation tests for input format, concurrency limits, resume/output path policy, and unsupported remote inputs.
- `clankers-session` tests for trajectory/ShareGPT serialization once added.
- Integration test such as `tests/batch_trajectory_runner.rs` for a deterministic happy path and one failure path without contacting real providers.
- Verification commands should include `cargo fmt`, targeted `CARGO_TARGET_DIR=target cargo nextest run -p clankers batch --no-fail-fast`, any session export filters, `CARGO_TARGET_DIR=target cargo check --tests -p clankers-session -p clankers`, OpenSpec verify, and `git diff --check`.

## Risks

- Scope creep: batch execution, resumability, trajectory export, daemon parity, and RL export can each become independent features.
- Provider cost/safety: running many prompts concurrently must have explicit limits and should respect existing model/provider/account settings.
- Privacy: exported trajectories must not include credentials, environment variables, headers, or raw tool inputs that may contain secrets unless explicitly intended by the existing session export policy.
- Replay drift: result files should include enough normalized run metadata to resume/debug without depending on transient backend state.
