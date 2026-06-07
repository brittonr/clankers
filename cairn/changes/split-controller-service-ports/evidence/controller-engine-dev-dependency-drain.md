Artifact-Type: validation-log
Task-ID: I6,V5
Covers: r[remaining-coupling-drain.controller-service-ports.runtime-adapter], r[remaining-coupling-drain.controller-service-ports.behavior-validation], r[remaining-coupling-drain.controller-service-ports.closeout]
Status: pass

## Scope

Removed the production `clankers-engine` dependency from `clankers-controller`:

- `core_engine_composition.rs` keeps the runtime-visible accepted-prompt DTOs (`AcceptedPromptKind`, `AcceptedPromptStart`) independent of engine types.
- Engine submission fixtures (`EngineSubmissionPolicy`, `EngineSubmissionPlan`, `engine_submission_from_prompt_start`, and feedback reducer checks) are now `#[cfg(test)]`-only.
- `clankers-engine` moved to a controller dev-dependency for FCIS/regression fixtures only.

## Dependency result

`clankers-controller` normal workspace dependencies are now:

```text
["clanker-message", "clankers-agent", "clankers-core", "clankers-db", "clankers-protocol", "clankers-session"]
```

The controller production internal dependency count in the lego baseline decreased from 7 to 6.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-controller
cargo check -p clankers-controller -p clankers --tests
cargo test -p clankers-controller --test fcis_shell_boundaries
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0.
