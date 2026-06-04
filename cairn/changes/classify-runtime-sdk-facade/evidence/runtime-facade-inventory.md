# Runtime facade inventory evidence

Evidence-ID: classify-runtime-sdk-facade.runtime-facade-inventory
Artifact-Type: implementation-evidence
Task-ID: I1,I2,I3
Covers: remaining-coupling-drain.runtime-facade-classification, remaining-coupling-drain.runtime-facade-classification.owner-map, remaining-coupling-drain.runtime-facade-classification.promotion-gate, remaining-coupling-drain.runtime-public-api-rail, remaining-coupling-drain.runtime-public-api-rail.leakage, remaining-coupling-drain.runtime-public-api-rail.deterministic
Date: 2026-06-04
Status: PASS

## Decision

`clankers-runtime` is classified as a yellow application-edge composition facade, not a generic green SDK crate. The `session-ledger-resume` API remains a green-candidate subset inside the yellow facade until the `promote-session-ledger-green-sdk` change decides whether to split it into a green owner.

## Artifacts

- `policy/embedded-lego/runtime-facade-boundary.json` records the runtime facade classification, source-group owners, dependency allowlist, forbidden dependency fragments, and forbidden source tokens.
- `docs/src/generated/runtime-facade-api.md` is generated from actual `clankers-runtime` public Rust exports. Current inventory: 2038 rows, hash `23d7bdae1b888af29483e2814980b3ba03ab4c29093e3c32da394e96642870e7`.
- `scripts/check-runtime-facade-boundary.rs` parses the runtime source with `syn`, maps every public item to a policy group, checks classified dependencies, rejects forbidden daemon/TUI/provider/router/session/plugin/global-path source tokens, and verifies the generated inventory is fresh.
- `crates/clankers-runtime/src/boundary.rs` no longer owns a small denied-name list; it points runtime tests at the deterministic inventory rail.

## Command evidence

```text
scripts/check-runtime-facade-boundary.rs
ok: runtime facade boundary inventories clankers-runtime public API and dependency classifications
exit=0

cargo test -p clankers-runtime --lib public_api_boundary_rejects_transport_type_leakage
running 1 test
test tests::public_api_boundary_rejects_transport_type_leakage ... ok
exit=0
```
