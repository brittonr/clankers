## Phase 1: Implementation

- [x] [serial] I1: Inventory provider/router constructor sites, request projections, cache-key helpers, auth/probe paths, retry/cooldown policy, and stream-normalization entrypoints into a concern owner map. r[remaining-coupling-drain.provider-router-convergence.concern-owner-map] [covers=remaining-coupling-drain.provider-router-convergence.concern-owner-map] [evidence=evidence/provider-router-owners.md]
- [x] [serial] I2: Collapse or delegate one duplicated provider concern so `clankers-provider` compatibility code only translates DTOs/events/errors for that concern. r[remaining-coupling-drain.provider-router-convergence.adapter-delegation] [covers=remaining-coupling-drain.provider-router-convergence.adapter-delegation] [evidence=evidence/provider-router-owners.md]
- [x] [serial] I3: Refresh compatibility docs and provider-router ownership rails after the selected concern moves to one owner. r[remaining-coupling-drain.provider-router-convergence.concern-owner-map] [covers=remaining-coupling-drain.provider-router-convergence.concern-owner-map] [evidence=evidence/provider-router-owners.md]

## Phase 2: Verification

- [x] [serial] V1: Run `scripts/check-provider-router-boundary.rs`, focused provider/router request-shape and projection parity tests, and a runtime parser-entrypoint seam test if stream normalization is touched. r[remaining-coupling-drain.provider-router-convergence.adapter-delegation] [covers=remaining-coupling-drain.provider-router-convergence.adapter-delegation] [evidence=evidence/provider-router-owners.md]
- [x] [serial] V2: Run Cairn validation/gates, `git diff --check`, and aggregate embedded SDK acceptance if compatibility labels or generated inventory rows move. r[remaining-coupling-drain.provider-router-convergence.concern-owner-map] [covers=remaining-coupling-drain.provider-router-convergence.concern-owner-map] [evidence=evidence/validation-closeout.md]
