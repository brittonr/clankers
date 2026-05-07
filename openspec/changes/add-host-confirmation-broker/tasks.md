## Phase 1: Broker foundation

- [x] [serial] Write the host confirmation broker OpenSpec package. [covers=embeddable-confirmation-broker.interface] [evidence=openspec validate add-host-confirmation-broker --strict]
- [ ] [serial] Define the confirmation broker trait/service and request/decision/error types. [covers=embeddable-confirmation-broker.interface]
- [ ] [parallel] Add fail-closed behavior for absent, unavailable, timed-out, and cancelled brokers. [covers=embeddable-confirmation-broker.interface.fail-closed]
- [ ] [parallel] Add safe request summaries and metadata redaction. [covers=embeddable-confirmation-broker.safe-requests]

## Phase 2: Adapter parity

- [ ] [serial] Route at least one existing confirmation-required tool/action through the broker substrate. [covers=embeddable-confirmation-broker.adapter-parity]
- [ ] [parallel] Add negative tests proving actions do not execute before approval. [covers=embeddable-confirmation-broker.adapter-parity.no-bypass]
- [ ] [parallel] Document host confirmation broker integration for embedded apps. [covers=embeddable-confirmation-broker.interface]
