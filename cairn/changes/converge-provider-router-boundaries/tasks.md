# Tasks: Converge Provider Router Boundaries

## Phase 1: Inventory

- [ ] [serial] R1: Update the provider/router concern owner map for request body shaping, discovery, auth refresh/probe, routing/fallback/cooldown, retry, cache-key projection, stream normalization, and error mapping. r[remaining-coupling-drain.provider-router-boundary-collapse.owner-map] [covers=remaining-coupling-drain.provider-router-boundary-collapse.owner-map]

## Phase 2: Implementation

- [ ] [serial] I1: Keep local and RPC compatibility adapters as DTO/error/stream projections that delegate provider-native policy to router/backend owners. r[remaining-coupling-drain.provider-router-boundary-collapse.thin-adapters] [covers=remaining-coupling-drain.provider-router-boundary-collapse.thin-adapters]
- [ ] [serial] I2: Collapse duplicate provider request/event fields where practical or strengthen constructor-count and shared-field parity rails for any temporary duplicate DTOs. r[remaining-coupling-drain.provider-router-boundary-collapse.duplicate-dtos] [covers=remaining-coupling-drain.provider-router-boundary-collapse.duplicate-dtos]
- [ ] [serial] I3: Add explicit request/stream/cache fixtures that prove compatibility adapters preserve summaries, metadata, tool replay, and routing inputs without duplicating backend policy. r[remaining-coupling-drain.provider-router-boundary-collapse.fixtures] [covers=remaining-coupling-drain.provider-router-boundary-collapse.fixtures]

## Phase 3: Verification

- [ ] [serial] V1: Run provider/router responsibility rails, request-shape fixtures, constructor-count parity tests, and fake-routed-backend adapter tests. r[remaining-coupling-drain.provider-router-boundary-collapse.validation] [covers=remaining-coupling-drain.provider-router-boundary-collapse.validation]
- [ ] [serial] V2: Run `cargo check --tests` for provider/router callers, Cairn gates/validate, and `git diff --check` before closeout. r[remaining-coupling-drain.provider-router-boundary-collapse.closeout] [covers=remaining-coupling-drain.provider-router-boundary-collapse.closeout]
