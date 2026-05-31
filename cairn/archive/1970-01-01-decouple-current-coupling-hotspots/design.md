# Design: Decouple Current Coupling Hotspots

## Summary

This change defines a decoupling roadmap for the current hotspots. The strategy is behavior-preserving extraction: introduce a neutral contract, add focused parity or architecture rails, migrate one caller group, then remove the old direct dependency path.

## Decisions

### Decision: data crates must not depend on display/runtime shells

`clankers-config` should deserialize and validate data. TUI color/theme/keymap projection belongs in TUI-owned adapters. This keeps config usable by headless, daemon, embedded, and test contexts without terminal dependencies.

### Decision: agent turn logic depends on ports, not concrete infrastructure

`clankers-agent` should express turn orchestration in terms of model, tool, prompt, storage, hooks, usage, and skill ports. Concrete provider/router/auth/db/runtime/TUI systems are shell adapters.

### Decision: tool inventory is a catalog, not a single constructor function

The current `build_tiered_tools` function is too central. Tool ownership should be split by catalog section: core filesystem/process tools, orchestration tools, daemon/session tools, optional matrix tools, plugin tools, and extension/runtime tools. Each section owns its dependencies and registration rules.

### Decision: controller works in domain events before protocol projection

`SessionController` should accept domain/session commands and emit domain/session outcomes. `SessionCommand` and `DaemonEvent` remain wire DTOs at transport/client boundaries. Protocol conversion must stay centralized and testable.

### Decision: daemon sockets are an imperative shell around a session builder

Socket framing and control-plane IO should not construct sessions inline. A session builder owns resume resolution, model/system prompt selection, capability setup, actor spawn inputs, and registry update payloads.

### Decision: slash commands return effects

Slash handlers should parse input and return declarative effects such as local UI mutation, session command, plugin command, navigation request, or system message. Standalone, daemon attach, and remote attach paths then share the same effect interpreter.

### Decision: provider/router ownership is single-source

There should be exactly one owner for provider-native request shapes and exactly one owner for routing/fallback/cooldown. Adapters may translate DTOs but must not reimplement the same policy.

### Decision: runtime process jobs split contracts from adapters

Process-job DTOs and policy contracts should be independently testable. Native backend, pueue/systemd adapters, DB storage conversion, notification decisions, and agent-visible tool JSON projection should be separate seams.

### Decision: compatibility re-exports are temporary migration aids

Root `src/{agent,config,provider,plugin,session,util}` compatibility modules and broad root `pub use` exports should be removed once call sites import owning crates directly. New code must not add dependencies through compatibility wrappers.

## Sequencing

1. Start with config/TUI because the dependency direction is clearly wrong and has limited runtime behavior risk.
2. Split tool factory/catalog next, because it reduces pressure on daemon, slash, and agent seams.
3. Move slash commands to declarative effects and controller domain/protocol separation together enough to preserve attach parity.
4. Thin daemon socket/session construction after the controller/session command effects are stable.
5. Migrate provider/router and agent ports where request-shape fixtures already exist.
6. Decompose runtime process-job contracts after preserving current process tool receipts.
7. Remove compatibility re-exports only after architecture rails prove no in-repo call sites need them.

## Validation Strategy

Each implementation slice should add at least one focused behavior fixture and one boundary rail. The boundary rail should prefer Cargo metadata, Rust AST/import checks, or typed manifests over raw grep. Broad cargo/nextest validation is useful only after the focused seam checks pass.
