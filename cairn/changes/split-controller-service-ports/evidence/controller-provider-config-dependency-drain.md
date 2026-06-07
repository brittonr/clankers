Artifact-Type: validation-log
Task-ID: I4,V3
Covers: r[remaining-coupling-drain.controller-service-ports.inventory], r[remaining-coupling-drain.controller-service-ports.runtime-adapter], r[remaining-coupling-drain.controller-service-ports.behavior-validation]
Status: pass

## Scope

Drained two production dependency edges from `clankers-controller`:

- `clankers-provider` is now a dev-dependency only. Provider-native test doubles remain under `#[cfg(test)]`, while production controller code uses agent-owned `AgentModelService` and controller runtime/agent adapters instead of provider-native request types.
- `clankers-config` was removed from the controller manifest. `ControllerConfig` is controller-owned, and no production or test controller source imports `clankers_config`.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-controller
cargo check -p clankers-controller --tests
cargo check -p clankers
cargo check -p clankers --tests
cargo test -p clankers --no-run
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0.

## Result

The dependency-ownership baseline now records `clankers-controller` with 5 concrete production dependencies instead of 7. The removed production edges are `clankers-provider` and `clankers-config`.
