## Phase 1: Implementation

- [ ] [serial] I1: Inventory the neutral ledger subset in `clankers-runtime` and separate pure DTO/replay logic from runtime IDs, clocks, errors, storage, and resume shell behavior. r[session-resume-brick.green-ledger-core] [covers=session-resume-brick.green-ledger-core]
- [ ] [serial] I2: Move or extract `SessionLedgerEntry`, `SessionLedgerMessage`, replay metadata, and engine-message projection into a green SDK owner while keeping storage backends app-owned. r[session-resume-brick.green-ledger-core] [covers=session-resume-brick.green-ledger-core]
- [ ] [serial] I3: Update `clankers-runtime`, desktop session-ledger adapters, and daemon resume seed paths to consume the green ledger owner through explicit adapters. r[session-resume-brick.ledger-adapters] [covers=session-resume-brick.ledger-adapters]
- [ ] [serial] I4: Update embedded session-store and product-workbench examples to persist/replay through the promoted ledger API instead of product-local duplicate history DTOs. r[session-resume-brick.ledger-adapters] [covers=session-resume-brick.ledger-adapters]
- [ ] [serial] I5: Update dependency, API inventory, docs, policy, lockfile/build-plan/flake inputs as needed for the green ledger owner. r[session-resume-brick.green-ledger-core] [covers=session-resume-brick.green-ledger-core]

## Phase 2: Verification

- [ ] [serial] V1: Run session-resume runtime fixtures and both embedded session examples, proving restored ordered context and fail-closed missing/unsupported stores. r[session-resume-brick.ledger-adapters] [covers=session-resume-brick.ledger-adapters]
- [ ] [serial] V2: Run `scripts/check-session-resume-brick.rs`, `scripts/check-session-ledger-boundary.rs`, `scripts/check-embedded-sdk-deps.rs`, and `scripts/check-embedded-agent-sdk.rs`. r[session-resume-brick.green-ledger-core] [covers=session-resume-brick.green-ledger-core]
- [ ] [serial] V3: Run Cairn validation/gates for this change and `git diff --check`. r[session-resume-brick.green-ledger-core] [covers=session-resume-brick.green-ledger-core]
