## Phase 1: Message contract split

- [ ] [serial] I1: Inventory `clanker-message` public types as stable SDK contract, optional support, compatibility alias, experimental, or transcript internal. r[sdk-message-contract-boundary.inventory] [covers=sdk-message-contract-boundary.inventory]
- [ ] [serial] I2: Define the stable SDK message subset and document which transcript types remain compatibility/internal. r[sdk-message-contract-boundary.stable-subset.contracts] [covers=sdk-message-contract-boundary.stable-subset.contracts]
- [ ] [parallel] I3: Move, isolate, or relabel one transcript-internal family so it is not advertised as green SDK API. r[sdk-message-contract-boundary.transcript-internals.compatibility-only] [covers=sdk-message-contract-boundary.transcript-internals.compatibility-only]
- [ ] [parallel] I4: Update provider/controller/session adapters that expose transcript internals at reusable boundaries or add owner receipts. r[sdk-message-contract-boundary.transcript-internals.edge-owned] [covers=sdk-message-contract-boundary.transcript-internals.edge-owned]

## Phase 2: Verification

- [ ] [serial] V1: Add serialization compatibility fixtures for existing transcript internals and migration adapters. r[sdk-message-contract-boundary.verification.compat-fixtures] [covers=sdk-message-contract-boundary.verification.compat-fixtures]
- [ ] [serial] V2: Add source/API rails rejecting `AgentMessage`, shell transcript variants, generated IDs, or wall-clock timestamps in green SDK APIs except allowed adapters. r[sdk-message-contract-boundary.verification.boundary-rails] [covers=sdk-message-contract-boundary.verification.boundary-rails]
- [ ] [serial] V3: Run message crate tests, SDK API inventory, embedded examples, Cairn gates/validate, and relevant session/provider/controller parity tests. r[sdk-message-contract-boundary.verification] [covers=sdk-message-contract-boundary.verification]
