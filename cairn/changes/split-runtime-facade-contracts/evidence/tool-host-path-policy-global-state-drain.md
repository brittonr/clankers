Artifact-Type: validation-log
Task-ID: I4,V3
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.fail-closed-services], r[remaining-coupling-drain.runtime-facade-contract-split.validation]
Status: pass

## Scope

Removed singleton process-global policy state from `clankers-tool-host` path-policy checks:

- `check_path(...)` now evaluates the standard `PathPolicy` directly instead of reading a `OnceLock<PathPolicy>` service locator.
- `init_policy()` remains as a compatibility no-op for existing callers, but no longer hides mutable/global service state inside the reusable host crate.
- The FCIS boundary rail was updated for the already-completed agent hook-port drain, so it no longer requires the removed concrete `clankers_hooks::HookPipeline` path inside agent turn runtime files.

## Validation

Commands run from repository root:

```text
cargo test -p clankers-controller --test fcis_shell_boundaries
cargo test -p clankers-tool-host path_policy
```

Both commands exited 0.
