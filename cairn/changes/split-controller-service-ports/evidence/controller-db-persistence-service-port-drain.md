Artifact-Type: validation-log
Task-ID: I7,V6
Covers: r[remaining-coupling-drain.controller-service-ports.persistence-port], r[remaining-coupling-drain.controller-service-ports.behavior-validation], r[remaining-coupling-drain.controller-service-ports.closeout]
Status: pass

## Scope

Removed the production `clankers-db` dependency from `clankers-controller` by splitting concrete storage side effects out of controller policy:

- Added `ControllerPersistenceService` as a controller-owned side-effect port for optional search indexing and compaction-summary tool-result storage.
- Changed `persistence.rs` to update `SessionManager` and then call the injected service instead of importing `clankers_db` or opening DB/search stores directly.
- Added `DbControllerPersistenceService` at the root shell edge to project the controller port onto `clankers_db::Db` and optional `SearchIndex`.
- Moved session metrics reducer/DTO contracts into neutral `clanker-message::metrics`; `clankers-db` now re-exports those storage-free contracts for persistence.

## Dependency result

`clankers-controller` normal workspace dependencies are now:

```text
["clanker-message", "clankers-agent", "clankers-core", "clankers-protocol", "clankers-session"]
```

The controller production internal dependency count in the lego baseline decreased from 6 to 5, and concrete dependencies decreased from 4 to 3.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-controller
cargo check -p clankers-controller -p clankers --tests
cargo test -p clankers-controller --lib
cargo test -p clankers-controller --test fcis_shell_boundaries
cargo test -p clanker-message -p clankers-db --lib
scripts/check-controller-runtime-boundary.rs
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0.
