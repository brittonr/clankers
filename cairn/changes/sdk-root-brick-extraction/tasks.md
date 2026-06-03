## Phase 1: Root brick extraction

- [ ] [serial] I1: Inventory root `src/tools`, `src/modes`, `src/runtime_services.rs`, and slash/session-command modules by wiring, projection, adapter, or reusable policy ownership. r[sdk-root-brick-extraction.inventory] [covers=sdk-root-brick-extraction.inventory]
- [ ] [serial] I2: Select one root-owned reusable policy cluster and define its target workspace brick or neutral adapter owner. r[sdk-root-brick-extraction.brick-owner.selected-cluster] [covers=sdk-root-brick-extraction.brick-owner.selected-cluster]
- [ ] [serial] I3: Move the selected policy into its owner while leaving root as parser/wiring/projection only. r[sdk-root-brick-extraction.brick-owner.root-wiring-only] [covers=sdk-root-brick-extraction.brick-owner.root-wiring-only]
- [ ] [parallel] I4: Update owner receipts and SDK docs if the extracted brick is product-facing. r[sdk-root-brick-extraction.rails.owner-receipts] [covers=sdk-root-brick-extraction.rails.owner-receipts]

## Phase 2: Verification

- [ ] [serial] V1: Add focused tests for the extracted brick that run without root CLI/TUI/daemon assembly. r[sdk-root-brick-extraction.verification.brick-tests] [covers=sdk-root-brick-extraction.verification.brick-tests]
- [ ] [serial] V2: Add root parity tests proving existing desktop CLI/TUI/daemon behavior is unchanged for the moved policy. r[sdk-root-brick-extraction.verification.root-parity] [covers=sdk-root-brick-extraction.verification.root-parity]
- [ ] [serial] V3: Run focused brick/root tests, lego architecture rail, Cairn gates/validate, and embedded SDK acceptance slice when applicable. r[sdk-root-brick-extraction.verification] [covers=sdk-root-brick-extraction.verification]
