## Phase 1: Implementation

- [ ] [serial] I1: Inventory `clankers-runtime` public exports, dependencies, and source tokens by green/yellow/red classification. r[remaining-coupling-drain.runtime-facade-classification] [covers=remaining-coupling-drain.runtime-facade-classification]
- [ ] [serial] I2: Decide whether runtime remains yellow-only, exposes a documented green subset, or splits green DTOs into smaller crates; update SDK guide and lego policy accordingly. r[remaining-coupling-drain.runtime-facade-classification] [covers=remaining-coupling-drain.runtime-facade-classification]
- [ ] [serial] I3: Replace the hardcoded runtime public boundary guard with a deterministic public API/dependency rail and owner diagnostics. r[remaining-coupling-drain.runtime-public-api-rail] [covers=remaining-coupling-drain.runtime-public-api-rail]
- [ ] [serial] I4: Add or update fail-closed fixtures for provider/auth/plugin/process/prompt/session services so missing host injection never falls back to ambient desktop state. r[remaining-coupling-drain.runtime-fail-closed-defaults] [covers=remaining-coupling-drain.runtime-fail-closed-defaults]

## Phase 2: Verification

- [ ] [serial] V1: Run runtime public API/dependency rails plus `scripts/check-runtime-extension-service-matrix.rs` and `scripts/check-config-prompt-skill-services.rs`. r[remaining-coupling-drain.runtime-public-api-rail] [covers=remaining-coupling-drain.runtime-public-api-rail]
- [ ] [serial] V2: Run `scripts/check-provider-router-boundary.rs`, `scripts/check-embedded-agent-sdk.rs`, and any changed runtime facade tests. r[remaining-coupling-drain.runtime-fail-closed-defaults] [covers=remaining-coupling-drain.runtime-fail-closed-defaults]
- [ ] [serial] V3: Run Cairn validation/gates for this change and `git diff --check`. r[remaining-coupling-drain.runtime-facade-classification] [covers=remaining-coupling-drain.runtime-facade-classification]
