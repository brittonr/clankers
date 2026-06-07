Artifact-Type: validation-log
Task-ID: I5,V4
Covers: r[remaining-coupling-drain.controller-service-ports.projection-owners], r[remaining-coupling-drain.controller-service-ports.behavior-validation], r[remaining-coupling-drain.controller-service-ports.closeout]
Status: pass

## Scope

Removed the production `clankers-hooks` dependency from `clankers-controller`:

- Added controller-owned `ControllerHookService` / `ControllerHookPoint` / `ControllerHookPayload` contracts in `crates/clankers-controller/src/hooks.rs`.
- `ControllerConfig` now accepts an injected hook service instead of a concrete `HookPipeline`.
- Controller lifecycle and agent hook dispatch now emit neutral controller hook intents; concrete `clankers_hooks::HookPipeline` projection lives in the root app adapter `HookPipelineControllerHookService`.
- Daemon and interactive session assembly wrap concrete hook pipelines at the root edge before constructing controllers.
- Controller tests keep concrete hook pipeline fixtures as dev-only adapters.

## Dependency result

`clankers-controller` concrete production dependencies decreased from 5 to 4:

```text
["clankers-agent", "clankers-db", "clankers-protocol", "clankers-session"]
```

`clankers-hooks` remains available only as a controller dev-dependency for focused hook-ordering fixtures.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-controller
cargo check -p clankers-controller -p clankers --tests
cargo test -p clankers-controller controller_owned_prompt_hooks_lifecycle_notifications_and_tool_hooks_fire_in_order
cargo test -p clankers-controller --lib
cargo test -p clankers-controller --test fcis_shell_boundaries
scripts/check-controller-runtime-boundary.rs
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0.
