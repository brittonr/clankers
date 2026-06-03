## Phase 1: Implementation

- [x] [serial] I1: Inventory provider/router duplicate abstractions for request, stream, auth, discovery, routing, retry, and cost concerns. r[provider-router-abstraction-collapse.duplicate-inventory] [covers=provider-router-abstraction-collapse.duplicate-inventory]
- [x] [serial] I2: Select cache-key request projection and name `router_request_bridge` as the single policy owner plus adapter boundary. r[provider-router-abstraction-collapse.single-owner] [covers=provider-router-abstraction-collapse.single-owner]
- [x] [serial] I3: Refactor `RouterProvider::compute_cache_key(...)` so request/message shape delegates to `router_request_bridge` and only cache eligibility remains local. r[provider-router-abstraction-collapse.thin-adapter] [covers=provider-router-abstraction-collapse.thin-adapter]
- [x] [serial] I4: Update projection parity/literal-fixture rails for cache-key request projection. r[provider-router-abstraction-collapse.verification] [covers=provider-router-abstraction-collapse.verification]

## Phase 2: Verification

- [x] [serial] V1: Run focused provider/router adapter fixtures for the selected concern. r[provider-router-abstraction-collapse.verification] [covers=provider-router-abstraction-collapse.verification] [evidence=evidence/cache-key-projection.md]
- [x] [serial] V2: Run provider/router cargo checks, relevant request-contract tests, Cairn gates/validate, and `git diff --check`. r[provider-router-abstraction-collapse.verification] [covers=provider-router-abstraction-collapse.verification] [evidence=evidence/cache-key-projection.md]
