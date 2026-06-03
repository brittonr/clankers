## Phase 1: Implementation

- [x] [serial] I1: Inventory brittle source anchors in `scripts/check-lego-architecture-boundaries.rs` and group them by ownership concern. r[typed-architecture-rail-hardening.anchor-inventory] [covers=typed-architecture-rail-hardening.anchor-inventory]
- [x] [serial] I2: Select the session-command-policy cluster and replace exact anchors with AST enum, field, return-type, function-body path, use-path, and fixture-function validation. r[typed-architecture-rail-hardening.typed-checks] [covers=typed-architecture-rail-hardening.typed-checks]
- [x] [serial] I3: Update rail diagnostics and baseline output to name source owner, target owner, and expected replacement path. r[typed-architecture-rail-hardening.diagnostics] [covers=typed-architecture-rail-hardening.diagnostics]
- [x] [serial] I4: Document exact-string fallback status for the selected cluster as replaced in the generated baseline `source_anchor_inventory`. r[typed-architecture-rail-hardening.anchor-inventory] [covers=typed-architecture-rail-hardening.anchor-inventory]

## Phase 2: Verification

- [x] [serial] V1: Run the hardened architecture rail and focused tests for the selected session-command-policy behavior fixture. r[typed-architecture-rail-hardening.verification] [covers=typed-architecture-rail-hardening.verification] [evidence=evidence/session-command-policy-rail.md]
- [x] [serial] V2: Run Cairn gates/validate and `git diff --check`; avoid broad rustfmt churn in cargo-script files. r[typed-architecture-rail-hardening.verification] [covers=typed-architecture-rail-hardening.verification] [evidence=evidence/session-command-policy-rail.md]
