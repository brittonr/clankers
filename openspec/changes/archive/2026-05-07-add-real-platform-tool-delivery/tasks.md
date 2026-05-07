## Phase 1: Spec Foundation

- [x] Write proposal, design, tasks, and delta spec for real platform Tool Gateway delivery.
- [x] Validate the OpenSpec package with `openspec validate add-real-platform-tool-delivery --strict` and record follow-up findings.

## Phase 2: Implementation

- [x] Inventory current Tool Gateway, Matrix bridge, scheduled output, and artifact-producing tool seams; record exact integration points in `verification.md`.
- [x] Add typed delivery request, adapter, outbox, attempt, and receipt models with redaction-focused unit tests.
- [x] Implement local/session delivery as an adapter-backed path rather than receipt-only metadata.
- [x] Implement Matrix delivery for explicit active Matrix bridge/session contexts with fake bridge tests and unsupported-context negative tests.
- [x] Wire scheduled-job output and generated media/file artifact handoff through the shared delivery boundary without bypassing policy.
- [x] Add CLI/tool actions for delivery status and retry that operate on outbox attempt ids rather than raw destinations.
- [x] Update README and reference docs with supported targets, non-goals, retry semantics, and redaction guarantees.

## Phase 3: Verification and Closeout

- [x] Run focused Tool Gateway and Matrix delivery unit/integration tests.
- [x] Run at least one deterministic end-to-end smoke that produces an artifact, delivers through a fake/platform adapter, and records a safe receipt.
- [x] Run `cargo check --tests`, `openspec validate add-real-platform-tool-delivery --strict`, and `git diff --check`.
- [x] Sync the delta spec into canonical specs and archive the change after implementation tasks complete.
