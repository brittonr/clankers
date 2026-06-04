## Phase 1: Spec Foundation

- [x] Write proposal, design, tasks, and delta spec for `add-automatic-checkpoints-rollback`.
- [x] Validate the OpenSpec package with `openspec validate add-automatic-checkpoints-rollback --strict` and record any follow-up findings.

## Phase 2: Implementation

- [x] Inventory current `checkpoints-rollback` code/docs seams and record the exact files to touch. Evidence: `verification.md#inventory`.
- [x] Add typed policy/config/request/receipt models with unit tests. Evidence: `src/checkpoints.rs` auto-checkpoint model tests.
- [x] Implement the first runtime/adapter slice behind deterministic fake tests. Evidence: `ensure_pre_mutation_checkpoint` strict/best-effort/create tests.
- [x] Wire the feature through the shared clankers surface without bypassing daemon/session/tool policy. Evidence: `write`, `edit`, and `patch` use shared `protect_file_mutation` before writes.
- [x] Update README and relevant docs for supported behavior, non-goals, and safety policy. Evidence: README working-directory checkpoints section.

## Phase 3: Verification and Closeout

- [x] Run targeted package/integration checks for the touched modules. Evidence: `verification.md#drain-verification-matrix`.
- [x] Run `cargo check --tests` for affected crates. Evidence: `verification.md#drain-verification-matrix`.
- [x] Run `git diff --check`. Evidence: `verification.md#drain-verification-matrix`.
- [x] Sync the delta spec into the canonical `checkpoints-rollback` spec and archive the change after implementation tasks complete. Evidence: archived change and canonical `openspec/specs/checkpoints-rollback/spec.md` validation.
