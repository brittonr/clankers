## Phase 1: Implementation

- [ ] [serial] I1: Inventory provider/router duplicate abstractions for request, stream, auth, discovery, routing, retry, and cost concerns. r[provider-router-abstraction-collapse.duplicate-inventory] [covers=provider-router-abstraction-collapse.duplicate-inventory]
- [ ] [serial] I2: Select one duplicated concern and name the single policy owner plus adapter boundary. r[provider-router-abstraction-collapse.single-owner] [covers=provider-router-abstraction-collapse.single-owner]
- [ ] [serial] I3: Refactor the selected compatibility path so policy delegates to the single owner and only DTO/error/stream translation remains. r[provider-router-abstraction-collapse.thin-adapter] [covers=provider-router-abstraction-collapse.thin-adapter]
- [ ] [serial] I4: Update constructor-count, projection parity, or literal-fixture rails for the selected concern. r[provider-router-abstraction-collapse.verification] [covers=provider-router-abstraction-collapse.verification]

## Phase 2: Verification

- [ ] [serial] V1: Run focused provider/router adapter fixtures for the selected concern. r[provider-router-abstraction-collapse.verification] [covers=provider-router-abstraction-collapse.verification]
- [ ] [serial] V2: Run provider/router cargo checks, relevant request-contract tests, Cairn gates/validate, and `git diff --check`. r[provider-router-abstraction-collapse.verification] [covers=provider-router-abstraction-collapse.verification]
