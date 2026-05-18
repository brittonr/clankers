## Phase 1: API contract

- [x] [serial] [covers=process-job-tool-api.requests.start-options] Define backend-neutral request DTOs for start/list/poll/log/wait/kill/restart/stdin/close/profile/adopt/GC. Evidence: `cargo test -p clankers-runtime process_jobs` passed at 2026-05-18T02:08:29Z.
- [x] [parallel] [covers=process-job-tool-api.receipts.common-shape] Define common receipt/error DTO fields and operation-specific payloads. Evidence: `cargo test -p clankers-runtime process_jobs` passed at 2026-05-18T02:28:54Z with `ProcessJobToolReceipt` common/payload envelope coverage.
- [x] [parallel] [covers=process-job-tool-api.identity.blake3-native] Define canonical BLAKE3 `ProcessJobId` envelope, digest encoding, legacy projection behavior, and backend-ref separation. Evidence: BLAKE3 fixture test in `clankers-runtime::process_jobs` passed at 2026-05-18T02:08:29Z.
- [x] [parallel] [covers=process-job-tool-api.compat.native-default] Inventory existing `process` parameters and map them into the new request DTOs without changing default native behavior. Evidence: `cargo test -p clankers --lib process:: -- --nocapture --test-threads=1` passed at 2026-05-18T02:08:29Z.

## Phase 2: Parser and projection seams

- [x] [serial] [depends:phase-1] Refactor `process` parser to produce request DTOs and call `ProcessJobService` rather than concrete backend/storage code. Evidence: `cargo test -p clankers --lib process_parser_produces_backend_neutral_request_dtos_for_all_actions -- --nocapture`, `cargo test -p clankers --lib process:: -- --nocapture --test-threads=1`, `cargo test -p clankers-runtime process_jobs -- --nocapture`, and `CARGO_TARGET_DIR=target/process-job-api-check cargo check -p clankers --tests` passed at 2026-05-18T03:27:56Z; non-start actions now parse through `ProcessJobToolRequest` before dispatching native/pueue/systemd service seams.
- [x] [parallel] [covers=process-job-tool-api.errors.unsupported-action] Add typed unsupported-action/backend-unavailable/capability-denied error receipts. Evidence: `cargo test -p clankers-runtime process_jobs -- --nocapture` and `cargo test -p clankers --lib process:: -- --nocapture --test-threads=1` passed at 2026-05-18T02:39:09Z.
- [x] [parallel] [covers=process-job-tool-api.receipts.projection] Add projection adapters for agent text, TUI/process panel data, and daemon/remote event payloads. Evidence: `cargo test -p clankers --lib process:: -- --nocapture --test-threads=1` and `cargo test -p clankers-runtime process_jobs -- --nocapture` passed at 2026-05-18T02:54:47Z with start/list/log/GC routed through shared `ProcessJobToolResult -> ProcessJobToolReceipt` envelopes.

## Phase 3: Verification

- [ ] [serial] [depends:phase-2] Add golden request/receipt serialization tests.
- [ ] [serial] [depends:phase-2] Add deterministic BLAKE3 identity fixture tests covering native, pueue, systemd, and legacy/sequential ID projection.
- [ ] [serial] [depends:phase-2] Add native compatibility tests for current process actions and defaults.
- [ ] [serial] [depends:phase-2] Run focused process tool tests, DTO tests, `openspec validate define-process-job-tool-api --strict --json`, and `git diff --check`.
