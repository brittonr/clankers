## Phase 1: Contract and fixture shape

- [x] [serial] [covers=session-metrics-capture.observability-audit-receipt-kit.boundary] [evidence=openspec validate brick-08-observability-audit-receipt-kit --strict --json] Finalize the proposal, design, and delta spec for `observability-audit-receipt-kit`.
- [x] [serial] [covers=session-metrics-capture.observability-audit-receipt-kit.boundary] [evidence=source anchor readback: crates/clankers-controller/src/metrics_capture.rs; crates/clankers-controller/src/audit.rs] Identify the minimal source anchors and decide whether the brick is an example, policy/manifest, generated inventory, receipt validator, focused test, or a combination.

## Phase 2: Implementation evidence

- [x] [serial] [covers=session-metrics-capture.observability-audit-receipt-kit.evidence] [evidence=cargo test -p clankers-controller observability_audit_receipt_kit_bounds_and_redacts_tool_state] Implement the narrowest deterministic brick evidence for `observability-audit-receipt-kit` with at least one positive path.
- [x] [parallel] [covers=session-metrics-capture.observability-audit-receipt-kit.evidence] [evidence=observability_audit_receipt_kit_bounds_and_redacts_tool_state redaction and over-limit assertions] Add one fail-closed, denial, drift, or redaction case for the brick.
- [x] [parallel] [covers=session-metrics-capture.observability-audit-receipt-kit.drift] [evidence=docs/src/reference/request-lifecycle.md; scripts/check-observability-audit-receipt-kit.rs; scripts/check-embedded-agent-sdk.sh] Update docs, policy, generated inventory, or receipt schemas that advertise the brick.

## Phase 3: Validation and archive

- [x] [depends:implementation] [covers=session-metrics-capture.observability-audit-receipt-kit.evidence] [evidence=2026-05-19T03:10:35Z: ./scripts/check-observability-audit-receipt-kit.rs; cargo test -p clankers-controller observability_audit_receipt_kit_bounds_and_redacts_tool_state] Run the focused verification for `observability-audit-receipt-kit` and capture the command in the archive note.
- [x] [depends:implementation] [covers=session-metrics-capture.observability-audit-receipt-kit.drift] [evidence=2026-05-19T03:10:35Z: cargo fmt --check && git diff --check] Run formatting and whitespace checks.
- [x] [depends:implementation] [covers=session-metrics-capture.observability-audit-receipt-kit.boundary] [evidence=2026-05-19T03:10:35Z: openspec validate session-metrics-capture --strict --json] Promote the spec delta, validate the canonical spec, and archive the change when complete.
