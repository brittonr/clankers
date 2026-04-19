# Actor Crate Extraction (erlactor)

## Purpose

Extract `clankers-actor` into a standalone Erlang-style actor library for
tokio. Provides process registry, signals, linking, monitors, and
supervision — patterns from Erlang/OTP adapted for native async Rust.

The crate has three dependencies (tokio, dashmap, tracing), zero workspace
dependencies, and one incidental "clankers" reference in a doc comment.

## Requirements

### Crate identity

r[actor.identity.name]
The extracted crate MUST be named `erlactor` (or chosen alternative).

r[actor.identity.repo]
The crate MUST live in its own GitHub repository.

### Source migration

r[actor.source.files]
The following files MUST be moved to the new repo:

- `src/lib.rs` — module declarations and re-exports
- `src/process.rs` — `ProcessId`, `ProcessHandle`, `DeathReason`
- `src/registry.rs` — `ProcessRegistry` (spawn, lookup, link, monitor, shutdown)
- `src/signal.rs` — `Signal` enum (Kill, Shutdown, LinkDied, Custom)
- `src/supervisor.rs` — `Supervisor`, `SupervisorConfig`, `SupervisorStrategy`

r[actor.source.no-clankers-refs]
The source MUST NOT contain the string "clankers" in any source file,
doc comment, or test.

r[actor.source.docs]
The crate root documentation MUST describe the Erlang-style primitives
offered and include a working doc-test that spawns a named process,
links two processes, and demonstrates cascading death.

### API surface

r[actor.api.registry]
The crate MUST export `ProcessRegistry` with at minimum:
- `new() -> Self`
- `spawn(name, future) -> ProcessId`
- `spawn_opts(name, future, die_when_link_dies) -> ProcessId`
- `lookup(name) -> Option<ProcessId>`
- `link(a, b)`
- `unlink(a, b)`
- `monitor(watcher, watched)`
- `send_signal(id, signal)`
- `shutdown_all(timeout)`

r[actor.api.process]
The crate MUST export `ProcessId`, `ProcessHandle`, `DeathReason`.

r[actor.api.signal]
The crate MUST export `Signal` with variants for Kill, Shutdown,
LinkDied, and extensible Custom signals.

r[actor.api.supervisor]
The crate MUST export `Supervisor`, `SupervisorConfig`, `SupervisorStrategy`
with OneForOne and OneForAll restart strategies.

### Linking semantics

r[actor.linking.die-when-link-dies]
When `die_when_link_dies` is true for process A, and A's linked process B
dies with a non-Normal reason, A MUST be killed automatically.

GIVEN process A spawned with `die_when_link_dies = true`
AND process A is linked to process B
WHEN process B terminates with `DeathReason::Failed`
THEN process A MUST receive a Kill signal and terminate

r[actor.linking.monitor-notification]
When a monitored process dies, all monitors MUST receive a `LinkDied`
signal with the dead process's ID and reason.

GIVEN process W monitors process X
WHEN process X terminates
THEN process W MUST receive `Signal::LinkDied { id: X, reason, tag }`

r[actor.linking.no-cascade-normal]
Normal termination MUST NOT cascade through links.

GIVEN process A linked to process B with `die_when_link_dies = true`
WHEN process B terminates with `DeathReason::Normal`
THEN process A MUST NOT be killed

### Tests

r[actor.tests.existing]
All existing tests (unit + `tests/integration.rs`) MUST pass in the
extracted crate.

r[actor.tests.linking]
The extracted crate MUST include tests for:
- Cascading death through links
- Monitor notification delivery
- `die_when_link_dies` = false prevents cascade
- Named process lookup after spawn
- Shutdown ordering (LIFO)

### Workspace migration

r[actor.migration.re-export]
After extraction, `crates/clankers-actor/` MUST become a thin wrapper
with a git dep on the new repo:
```rust
pub use erlactor::*;
```

r[actor.migration.callers-unchanged]
All 14 call sites in the workspace (daemon, matrix bridge, subagent tool,
tests, verus proofs) MUST compile without changes.

r[actor.migration.workspace-builds]
`cargo check` and `cargo nextest run` MUST pass on the full workspace.
