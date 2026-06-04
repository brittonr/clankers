## Phase 1: Implementation

- [ ] [serial] I1: Generate an experimental SDK port budget grouped by crate, owner module, current use sites, and proposed disposition. r[embedded-composition-kits.experimental-port-budget] [covers=embedded-composition-kits.experimental-port-budget]
- [ ] [serial] I2: Resolve unused `clankers-engine-host` experimental ports by either wiring/documenting them with fixtures or making them private. r[embedded-composition-kits.experimental-port-budget] [covers=embedded-composition-kits.experimental-port-budget]
- [ ] [serial] I3: Select representative `clankers-tool-host` context/service APIs and dogfood them through deterministic positive and fail-closed fixtures before promotion. r[neutral-tool-context.supported-service-ports] [covers=neutral-tool-context.supported-service-ports]
- [ ] [serial] I4: Refresh inventory labels, docs, and brick stability counts/hashes to reflect promoted, retained-experimental, and hidden items. r[embedded-composition-kits.experimental-port-budget] [covers=embedded-composition-kits.experimental-port-budget]

## Phase 2: Verification

- [ ] [serial] V1: Run focused tool-host/engine-host fixtures for each promoted or hidden port group. r[neutral-tool-context.supported-service-ports] [covers=neutral-tool-context.supported-service-ports]
- [ ] [serial] V2: Run `scripts/check-embedded-sdk-api.rs`, `scripts/check-brick-inventory-stability.rs`, `scripts/check-tool-catalog-matrix.rs`, `scripts/check-engine-host-feature-matrix.rs`, and `scripts/check-embedded-agent-sdk.rs`. r[embedded-composition-kits.experimental-port-budget] [covers=embedded-composition-kits.experimental-port-budget]
- [ ] [serial] V3: Run Cairn validation/gates for this change and `git diff --check`. r[embedded-composition-kits.experimental-port-budget] [covers=embedded-composition-kits.experimental-port-budget]
