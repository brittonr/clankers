## Phase 1: Session ledger boundary

- [ ] [serial] I1: Inventory desktop session setup, restore, merge, replay, controller persistence, and runtime ledger paths by DTO owner. r[sdk-session-ledger-boundary.inventory] [covers=sdk-session-ledger-boundary.inventory]
- [ ] [serial] I2: Select one restore/resume path and move reusable behavior behind neutral ledger/session-store DTOs. r[sdk-session-ledger-boundary.ledger-boundary.selected-path] [covers=sdk-session-ledger-boundary.ledger-boundary.selected-path]
- [ ] [parallel] I3: Keep `clankers-session`/`AgentMessage` storage handling inside desktop compatibility adapters. r[sdk-session-ledger-boundary.desktop-compat.adapter-owned] [covers=sdk-session-ledger-boundary.desktop-compat.adapter-owned]
- [ ] [parallel] I4: Update SDK docs/examples so embedders use host-owned ledger DTOs rather than Clankers session stores. r[sdk-session-ledger-boundary.ledger-boundary.sdk-owned-store] [covers=sdk-session-ledger-boundary.ledger-boundary.sdk-owned-store]

## Phase 2: Verification

- [ ] [serial] V1: Add or update session-resume brick fixtures covering restored user/tool/assistant context and missing-session fail-closed behavior. r[sdk-session-ledger-boundary.verification.resume-fixtures] [covers=sdk-session-ledger-boundary.verification.resume-fixtures]
- [ ] [serial] V2: Add desktop restore/attach replay parity tests for timestamps, finalized hashes, tool results, branch/compaction context, and semantic event conversion. r[sdk-session-ledger-boundary.verification.desktop-parity] [covers=sdk-session-ledger-boundary.verification.desktop-parity]
- [ ] [serial] V3: Run session/runtime/controller focused tests, session-resume brick checker, SDK dependency checks, Cairn gates/validate, and relevant attach replay tests. r[sdk-session-ledger-boundary.verification] [covers=sdk-session-ledger-boundary.verification]
