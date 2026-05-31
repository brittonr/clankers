# Root compatibility reexports rail evidence

Evidence-ID: root-compat-reexports-rails
Artifact-Type: command-output-summary
Task-ID: V9
Covers: coupling-hotspot-remediation.root-reexport-boundary
Date: 2026-05-31
Status: PASS

## Commands

```text
./scripts/check-root-compat-reexports.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clankers --tests
```

## Relevant output

```text
ok: root compatibility reexport rail passed

warning: `clankers` (lib) generated 4 warnings
Checking clankers v0.1.0 (/home/brittonr/git/clankers)
Finished `dev` profile [optimized + debuginfo] target(s) in 0.95s
```

## Coverage notes

The static rail verifies that root compatibility wrapper files for `agent`, `config`, `provider`, and `util` are removed, and that `src/lib.rs` no longer exposes root compatibility modules/reexports for extracted crates (`agent`, `config`, `provider`, `util`, `db`, `message`, `model_selection`, `procmon`, `tui`, and `clankers_session`). It also verifies `src/plugin/mod.rs` no longer re-exports `clankers-plugin` symbols and keeps only main-crate glue (`contributions` and protocol summary projection), while `src/session/mod.rs` keeps only the local merge-view adapter.

The rail scans `src/` and `tests/` for stale imports through root compatibility paths and plugin reexport paths. `cargo check -p clankers --tests` covers the binary, library tests, and integration-test imports after call sites were changed to use the owning crates directly.
