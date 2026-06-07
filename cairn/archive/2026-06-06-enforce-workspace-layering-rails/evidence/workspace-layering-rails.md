# Workspace layering rails evidence

Evidence-ID: enforce-workspace-layering-rails.workspace-layering-rails
Artifact-Type: command-output-summary
Task-ID: I1,I2,I3,V1
Covers: remaining-coupling-drain.architecture-rail-hardening.workspace-layer-map
Date: 2026-06-06
Status: PASS

## Implementation summary

- Added deterministic policy source `policy/workspace-layering/layers.json` assigning every workspace package to `green-contracts`, `host-facades`, `orchestration`, or `application-shells`.
- Added `scripts/check-workspace-layering-rails.rs`, a Cargo metadata plus Rust AST rail that:
  - verifies every workspace package is assigned exactly once;
  - rejects non-dev dependency edges from a lower-ranked layer to a higher-ranked layer unless listed in `allowed_upward_edges`;
  - scans green/host source AST paths outside test modules for higher-layer crate path references;
  - writes `target/workspace-layering/workspace-layering-inventory.json` with package/layer and dependency-edge inventory.
- Replaced the embedded SDK dependency rail's local workspace denied-crate list with policy-derived forbidden workspace crates above the embeddable layer rank; external forbidden crates remain explicit.
- Regenerated the standalone example lockfiles used by `scripts/check-embedded-sdk-deps.rs` so the rail stays `--locked` after `clankers-tool-host`'s existing `blake3` dependency.

## Commands completed

```text
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-workspace-layering-rails.rs
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-embedded-sdk-deps.rs
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller --test fcis_shell_boundaries
```

## Relevant output

```text
workspace layering inventory written to target/workspace-layering/workspace-layering-inventory.json
exit=0

ok: embedded SDK example dependency graph has 86 packages and excludes forbidden runtime crates
exit=0

running 44 tests
...
test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.65s
exit=0
```
