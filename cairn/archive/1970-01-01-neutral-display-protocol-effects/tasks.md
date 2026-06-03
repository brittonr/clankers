## Phase 1: Implementation

- [x] [serial] I1: Inventory slash/mode policy sites that construct `SessionCommand` or `clanker-tui-types` values before projection. r[neutral-display-protocol-effects.neutral-effects] [covers=neutral-display-protocol-effects.neutral-effects]
- [x] [serial] I2: Choose disabled-tools slash/effect handling and define neutral `SessionCommandIntent` data for it. r[neutral-display-protocol-effects.neutral-effects] [covers=neutral-display-protocol-effects.neutral-effects]
- [x] [serial] I3: Implement standalone, attach, and remote/daemon projection adapters for disabled-tools neutral effects. r[neutral-display-protocol-effects.projection-adapters] [covers=neutral-display-protocol-effects.projection-adapters]
- [x] [serial] I4: Update parity and source rails to reject disabled-tools protocol constructors in the selected policy owner. r[neutral-display-protocol-effects.verification] [covers=neutral-display-protocol-effects.verification]

## Phase 2: Verification

- [x] [serial] V1: Run focused slash/attach parity tests for the selected effect family. r[neutral-display-protocol-effects.verification] [covers=neutral-display-protocol-effects.verification] [evidence=evidence/disabled-tools-neutral-effects.md]
- [x] [serial] V2: Run architecture rails, `cargo check -p clankers --tests`, Cairn gates/validate, and `git diff --check`. r[neutral-display-protocol-effects.verification] [covers=neutral-display-protocol-effects.verification] [evidence=evidence/disabled-tools-neutral-effects.md]
