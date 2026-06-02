## Phase 1: Implementation

- [ ] [serial] I1: Inventory brittle source anchors in `scripts/check-lego-architecture-boundaries.rs` and group them by ownership concern. r[typed-architecture-rail-hardening.anchor-inventory] [covers=typed-architecture-rail-hardening.anchor-inventory]
- [ ] [serial] I2: Select one anchor cluster and replace it with AST, metadata, manifest, or behavior-based validation. r[typed-architecture-rail-hardening.typed-checks] [covers=typed-architecture-rail-hardening.typed-checks]
- [ ] [serial] I3: Update rail diagnostics and baseline output to name source owner, target owner, and expected replacement path. r[typed-architecture-rail-hardening.diagnostics] [covers=typed-architecture-rail-hardening.diagnostics]
- [ ] [serial] I4: Document any exact-string fallback left in the selected cluster. r[typed-architecture-rail-hardening.anchor-inventory] [covers=typed-architecture-rail-hardening.anchor-inventory]

## Phase 2: Verification

- [ ] [serial] V1: Run the hardened architecture rail and focused tests for any behavior fixture added. r[typed-architecture-rail-hardening.verification] [covers=typed-architecture-rail-hardening.verification]
- [ ] [serial] V2: Run Cairn gates/validate and `git diff --check`; avoid broad rustfmt churn in cargo-script files. r[typed-architecture-rail-hardening.verification] [covers=typed-architecture-rail-hardening.verification]
