## Phase 1: Broker foundation

- [x] [serial] Write the host confirmation broker OpenSpec package. [covers=embeddable-confirmation-broker.interface] [evidence=openspec validate add-host-confirmation-broker --strict]
- [x] [serial] Define the confirmation broker trait/service and request/decision/error types. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=embeddable-confirmation-broker.interface] [evidence=clankers_runtime::ConfirmationBroker]
- [x] [parallel] Add fail-closed behavior for absent, unavailable, timed-out, and cancelled brokers. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=embeddable-confirmation-broker.interface.fail-closed] [evidence=clankers-runtime::tests::confirmation_broker_fail_closed_for_absent_timeout_cancelled]
- [x] [parallel] Add safe request summaries and metadata redaction. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=embeddable-confirmation-broker.safe-requests] [evidence=clankers-runtime::tests::confirmation_request_metadata_redacts_secret_markers]

## Phase 2: Adapter parity

- [ ] [serial] Route at least one existing confirmation-required tool/action through the broker substrate. [covers=embeddable-confirmation-broker.adapter-parity]
- [ ] [parallel] Add negative tests proving actions do not execute before approval. [covers=embeddable-confirmation-broker.adapter-parity.no-bypass]
- [x] [parallel] Document host confirmation broker integration for embedded apps. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=embeddable-confirmation-broker.interface] [evidence=docs/src/reference/embedding.md]
