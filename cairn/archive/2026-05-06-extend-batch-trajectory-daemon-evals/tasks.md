## Phase 1: Spec Foundation

- [x] Write proposal, design, tasks, and delta spec for `extend-batch-trajectory-daemon-evals`.
- [x] Validate the OpenSpec package with `openspec validate extend-batch-trajectory-daemon-evals --strict` and record any follow-up findings.

## Phase 2: Implementation

- [x] Inventory current `batch-trajectory-runner` code/docs seams and record the exact files to touch. Evidence: `verification.md` inventory lists `src/modes/batch.rs`, `src/commands/batch.rs`, `src/cli.rs`, `tests/batch_trajectory_runner.rs`, README, quickstart, and request-lifecycle docs.
- [x] Add typed policy/config/request/receipt models with unit tests. Evidence: `BatchExecutionMode`, run ids, manifests, objective receipts, safe metadata, and batch unit tests in `src/modes/batch.rs`.
- [x] Implement the first runtime/adapter slice behind deterministic fake tests. Evidence: daemon/session manifest and eval JSONL integration tests in `tests/batch_trajectory_runner.rs`.
- [x] Wire the feature through the shared clankers surface without bypassing daemon/session/tool policy. Evidence: `src/commands/batch.rs` maps daemon jobs to deterministic session ids and invokes headless prompts with `--resume <session-id>` through the normal clankers prompt/session path.
- [x] Update README and relevant docs for supported behavior, non-goals, and safety policy. Evidence: README, quickstart, and request lifecycle batch sections updated.

## Phase 3: Verification and Closeout

- [x] Run targeted package/integration checks for the touched modules. Evidence: `cargo test --lib batch` and `cargo test --test batch_trajectory_runner` passed; details in `verification.md`.
- [x] Run `cargo check --tests` for affected crates. Evidence: passed; details in `verification.md`.
- [x] Run `git diff --check`. Evidence: passed; details in `verification.md`.
- [x] Sync the delta spec into the canonical `batch-trajectory-runner` spec and archive the change after implementation tasks complete. Evidence: `openspec archive extend-batch-trajectory-daemon-evals --yes` archived this change to `openspec/changes/archive/2026-05-06-extend-batch-trajectory-daemon-evals/` and updated `openspec/specs/batch-trajectory-runner/spec.md`.
