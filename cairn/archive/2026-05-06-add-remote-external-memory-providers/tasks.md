## Phase 1: Spec Foundation

- [x] Write proposal, design, tasks, and delta spec for `add-remote-external-memory-providers`.
- [x] Validate the OpenSpec package with `openspec validate add-remote-external-memory-providers --strict` and record any follow-up findings.

## Phase 2: Implementation

- [x] Inventory current `external-memory-providers` code/docs seams and record the exact files to touch. Evidence: `verification.md#inventory`.
- [x] Add typed policy/config/request/receipt models with unit tests. Evidence: `verification.md#drain-verification-matrix` covers config validation plus remote request/response structs and metadata receipts.
- [x] Implement the first runtime/adapter slice behind deterministic fake tests. Evidence: deterministic local TCP HTTP-provider test in `src/tools/external_memory.rs`.
- [x] Wire the feature through the shared clankers surface without bypassing daemon/session/tool policy. Evidence: existing `external_memory` Specialty tool publication uses validated `ExternalMemorySettings` and `ToolContext`.
- [x] Update README and relevant docs for supported behavior, non-goals, and safety policy. Evidence: README and `docs/src/reference/config.md`.

## Phase 3: Verification and Closeout

- [x] Run targeted package/integration checks for the touched modules. Evidence: `verification.md#drain-verification-matrix`.
- [x] Run `cargo check --tests` for affected crates. Evidence: `verification.md#drain-verification-matrix`.
- [x] Run `git diff --check`. Evidence: `verification.md#drain-verification-matrix`.
- [x] Sync the delta spec into the canonical `external-memory-providers` spec and archive the change after implementation tasks complete. Evidence: archived change plus post-archive `openspec validate external-memory-providers --strict`.
