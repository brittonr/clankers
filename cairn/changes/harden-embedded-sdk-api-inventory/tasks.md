## Phase 1: Implementation

- [ ] [serial] I1: Define the typed inventory schema for public SDK top-level items, methods, public fields, modules, constants, traits, type aliases, and root reexports. r[embedded-composition-kits.api-inventory-typed] [covers=embedded-composition-kits.api-inventory-typed]
- [ ] [serial] I2: Replace or augment `scripts/check-embedded-sdk-api.rs` with a typed parser that ignores test-only items without truncating runtime source. r[embedded-composition-kits.api-inventory-typed] [covers=embedded-composition-kits.api-inventory-typed]
- [ ] [serial] I3: Refresh `docs/src/generated/embedded-sdk-api.md` and `policy/embedded-lego/brick-inventory-stability.json` with explicit count/hash changes. r[embedded-composition-kits.api-inventory-stability] [covers=embedded-composition-kits.api-inventory-stability]
- [ ] [serial] I4: Add fixture/self-test coverage showing public methods, public fields, reexports, feature-gated items, and test-only APIs are handled correctly. r[embedded-composition-kits.api-inventory-typed] [covers=embedded-composition-kits.api-inventory-typed]

## Phase 2: Verification

- [ ] [serial] V1: Run `scripts/check-embedded-sdk-api.rs` and `scripts/check-brick-inventory-stability.rs` with the refreshed inventory. r[embedded-composition-kits.api-inventory-stability] [covers=embedded-composition-kits.api-inventory-stability]
- [ ] [serial] V2: Run `scripts/check-embedded-agent-sdk.rs` and confirm release receipt hashing includes the updated inventory policy. r[embedded-composition-kits.api-inventory-stability] [covers=embedded-composition-kits.api-inventory-stability]
- [ ] [serial] V3: Run Cairn validation/gates for this change and `git diff --check`. r[embedded-composition-kits.api-inventory-stability] [covers=embedded-composition-kits.api-inventory-stability]
