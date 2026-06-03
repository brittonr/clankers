Task-ID: I1,I2,I3,I4,V1,V2,V3
Covers: sdk-controller-runtime-boundary.inventory,sdk-controller-runtime-boundary.runtime-adapter.production-injection,sdk-controller-runtime-boundary.persistence.service-owned,sdk-controller-runtime-boundary.projection.centralized,sdk-controller-runtime-boundary.verification.fake-runtime,sdk-controller-runtime-boundary.verification.agent-parity,sdk-controller-runtime-boundary.verification
Artifact-Type: validation-evidence

# Controller Runtime Boundary Closeout

## Inventory / owners

`scripts/check-controller-runtime-boundary.rs` records the controller owner map:

- `crates/clankers-controller/src/lib.rs`: compatibility shell fields (`Agent`, `SessionManager`, hooks, outgoing `DaemonEvent`, search index) plus convergence comments.
- `crates/clankers-controller/src/runtime_adapter.rs`: `ControllerRuntimeAdapter`, fake runtime, and agent-backed runtime owner.
- `crates/clankers-controller/src/command.rs`: selected prompt/control command lifecycle through injected adapter seams (`submit_prompt_with_runtime_adapter`, `apply_control_with_runtime_adapter`, `handle_command_with_runtime_adapter_for_test`).
- `crates/clankers-controller/src/persistence.rs`: compatibility persistence/search adapter.
- `crates/clankers-controller/src/convert.rs`: projection owner for daemon/TUI/semantic conversions.

## Selected path

The selected migrated path is the production-compatible prompt/control command lifecycle already exposed through `ControllerRuntimeAdapter` fixtures. The fake-runtime path exercises prompt submission, abort/reset, thinking, disabled tools, resume/session identity, and semantic event projection without sockets, providers, TUI state, or desktop storage.

## Validation

Focused rails/tests:

- `nix develop -c cargo -q -Zscript scripts/check-controller-runtime-boundary.rs`
- `nix develop -c cargo nextest run -p clankers-controller runtime_adapter`

