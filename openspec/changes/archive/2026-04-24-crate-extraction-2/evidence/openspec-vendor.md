# OpenSpec Vendor Snapshot Evidence

Evidence-ID: ce2-openspec-vendor
Task-ID: 2a-openspec-source-pinning
Artifact-Type: implementation-evidence
Covers: specs-extraction, workspace-continuity
Created: 2026-04-24
Status: complete

## Decision

`brittonr/openspec` was not published when closing `crate-extraction-2`, and this session did not have an explicit push request. To remove non-reproducible sibling path dependencies without pushing to GitHub, clankers now vendors the extracted `openspec` working-tree snapshot under `vendor/openspec`.

The snapshot records its origin in `vendor/openspec/VENDORED_FROM`:

```text
source_repo=local:/home/brittonr/git/openspec
base_commit=effdd1b66aba0144f4bee69edf82241f60c86fe8
snapshot=working-tree
reason=brittonr/openspec remote was not published when clankers switched away from ../openspec path dependencies
```

## Implementation

- Root workspace dependency now declares `openspec = { path = "vendor/openspec" }`.
- Root crate uses `openspec = { workspace = true, optional = true }`.
- `crates/clankers-agent` uses `openspec = { workspace = true, optional = true }`.
- `Cargo.lock` was updated after switching source provenance; the `openspec` package now carries the vendored crate's dev dependency on `tempfile`.
- `flake.nix` no longer strips optional `openspec` dependencies before unit2nix evaluation.
- The vendored snapshot includes README, LICENSE, and GitHub Actions CI workflow scaffold for eventual publication.
- `build-plan.json` was regenerated after the source move so it points at `vendor/openspec`, not `/home/brittonr/git/openspec`.
- A deterministic search found no stale `../openspec` manifest dependency or flake-strip workaround in `Cargo.toml`, `crates/clankers-agent/Cargo.toml`, `flake.nix`, or `build-plan.json`.

## Validation

This session ran:

```text
$ cargo metadata --no-deps --format-version 1
/home/brittonr/git/clankers/vendor/openspec/Cargo.toml

$ git diff -- Cargo.lock
openspec dependencies gained `tempfile` after resolving the vendored source tree.

$ RUSTC_WRAPPER= cargo check --workspace
Finished `dev` profile [optimized + debuginfo] target(s) in 2m 28s

$ RUSTC_WRAPPER= cargo test --manifest-path vendor/openspec/Cargo.toml
61 unit tests passed, 2 integration tests passed, doc-tests passed

$ RUSTC_WRAPPER= cargo test --manifest-path vendor/openspec/openspec-plugin/Cargo.toml
3 runtime tests passed through Extism

$ unit2nix --workspace --force --no-check -o build-plan.json
Wrote build-plan.json

$ rg '/home/brittonr/git/openspec|path = "\\.\\./openspec"|\\.\\./\\.\\./\\.\\./openspec' Cargo.toml crates/clankers-agent/Cargo.toml flake.nix build-plan.json
(no matches)

$ nix build .#clankers -L --no-link
Succeeded in pueue task 47 after staging the vendored source tree.

$ openspec validate crate-extraction-2
Change 'crate-extraction-2' is valid
```
