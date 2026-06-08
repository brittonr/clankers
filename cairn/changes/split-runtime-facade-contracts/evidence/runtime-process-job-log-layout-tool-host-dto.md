Artifact-Type: validation-log
Task-ID: I31,V30
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved native process/job log layout DTOs to the neutral tool-host process/job owner:

- Added `clankers_tool_host::process_jobs::NativeProcessJobLogLayout` with safe log-reference projection.
- Re-exported that contract through `clankers-runtime::process_jobs` so existing runtime public API paths remain available.
- Kept log retention policy, retention timestamp projection, runtime log stores, backend services, and runtime error handling in `clankers-runtime`; only the backend-neutral append-only layout/reference data moved to the existing process/job contract crate.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-tool-host -p clankers-runtime -p clankers --tests
cargo test -p clankers-tool-host native_log_layout_sanitizes_references_without_host_io --lib
cargo test -p clankers-runtime native_log_layout_is_append_only_bounded_and_safe --lib
scripts/check-runtime-facade-boundary.rs --write-inventory
scripts/check-runtime-facade-boundary.rs
cargo -q -Zscript scripts/check-runtime-facade-split.rs
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
git diff --check
cargo test -p clankers --no-run
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0.
