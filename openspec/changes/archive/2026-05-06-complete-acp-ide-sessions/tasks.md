## Phase 1: Spec Foundation

- [x] Write proposal, design, tasks, and delta spec for `complete-acp-ide-sessions`.
- [x] Validate the OpenSpec package with `openspec validate complete-acp-ide-sessions --strict` and record any follow-up findings.

## Phase 2: Implementation

- [x] Inventory current `acp-ide-integration` code/docs seams and record the exact files to touch. Evidence: `verification.md#inventory`.
- [x] Add typed policy/config/request/receipt models with unit tests. Evidence: `src/modes/acp.rs` and `CARGO_TARGET_DIR=target cargo test --lib acp -- --nocapture`.
- [x] Implement the first runtime/adapter slice behind deterministic fake tests. Evidence: foreground JSON-line `initialize`, `session/new`, and `session/prompt` handling in `src/modes/acp.rs`; integration coverage in `tests/acp_ide_integration.rs`.
- [x] Wire the feature through the shared clankers surface without bypassing daemon/session/tool policy. Evidence: supported ACP paths only emit safe foreground stdio receipts; unsupported terminal/workspace/tool/diff surfaces fail closed with structured errors.
- [x] Update README and relevant docs for supported behavior, non-goals, and safety policy. Evidence: `README.md`, `docs/src/getting-started/quickstart.md`, `docs/src/reference/daemon.md`, and `docs/src/reference/request-lifecycle.md`.

## Phase 3: Verification and Closeout

- [x] Run targeted package/integration checks for the touched modules. Evidence: `verification.md#drain-verification-matrix`.
- [x] Run `cargo check --tests` for affected crates. Evidence: `verification.md#drain-verification-matrix`.
- [x] Run `git diff --check`. Evidence: `verification.md#drain-verification-matrix`.
- [x] Sync the delta spec into the canonical `acp-ide-integration` spec and archive the change after implementation tasks complete. Evidence: archived change and post-archive canonical validation.
