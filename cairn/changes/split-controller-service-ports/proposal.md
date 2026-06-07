# Change: Split Controller Service Ports

## Why

`clankers-controller` still depends directly on agent, config, DB, hooks, protocol, provider, and session crates. The current owner receipt says those edges are allowed only as separately testable translation, effect, runtime, persistence, continuation, and projection seams. That is still too coupled when command handling or event processing can reach concrete runtime, persistence, provider, or protocol types directly.

## What Changes

- Inventory controller concrete dependencies by responsibility: command translation, authorization, core input, runtime dispatch, persistence/search, hooks, continuation, and projection.
- Move agent/provider execution behind `ControllerRuntimeAdapter` and remove provider-native compatibility from command policy paths.
- Move DB/session persistence and search behavior behind a session service port so command/event logic does not open stores directly.
- Keep protocol and TUI/daemon DTO construction in `convert.rs` and `transport_convert.rs` with FCIS constructor rails.

## Impact

- **Files**: `crates/clankers-controller/src/{command.rs,command_*.rs,runtime_adapter.rs,persistence.rs,event_processing.rs,convert.rs,transport_convert.rs}`, FCIS/source-boundary tests, and root/daemon controller construction adapters.
- **Testing**: controller command/effect/runtime adapter tests, persistence service-port tests, FCIS shell-boundary rail, transport-construction rails, `cargo check --tests`, Cairn gates, and diff checks.
