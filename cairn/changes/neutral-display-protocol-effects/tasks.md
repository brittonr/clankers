## Phase 1: Implementation

- [ ] [serial] I1: Inventory slash/mode policy sites that construct `SessionCommand` or `clanker-tui-types` values before projection. r[neutral-display-protocol-effects.neutral-effects] [covers=neutral-display-protocol-effects.neutral-effects]
- [ ] [serial] I2: Choose one command/effect family and define neutral effect DTOs for it. r[neutral-display-protocol-effects.neutral-effects] [covers=neutral-display-protocol-effects.neutral-effects]
- [ ] [serial] I3: Implement standalone, attach, and remote/daemon projection adapters for the selected neutral effects. r[neutral-display-protocol-effects.projection-adapters] [covers=neutral-display-protocol-effects.projection-adapters]
- [ ] [serial] I4: Update parity and source rails to reject display/protocol constructors in the selected policy owner. r[neutral-display-protocol-effects.verification] [covers=neutral-display-protocol-effects.verification]

## Phase 2: Verification

- [ ] [serial] V1: Run focused slash/attach parity tests for the selected effect family. r[neutral-display-protocol-effects.verification] [covers=neutral-display-protocol-effects.verification]
- [ ] [serial] V2: Run architecture rails, `cargo check -p clankers --tests`, Cairn gates/validate, and `git diff --check`. r[neutral-display-protocol-effects.verification] [covers=neutral-display-protocol-effects.verification]
