Artifact-Type: validation-log
Task-ID: I8,V7
Covers: r[remaining-coupling-drain.controller-service-ports.persistence-port], r[remaining-coupling-drain.controller-service-ports.behavior-validation], r[remaining-coupling-drain.controller-service-ports.closeout]
Status: pass

## Scope

Removed the production `clankers-session` dependency from `clankers-controller` by replacing the raw `SessionManager` field/config edge with a controller-owned ledger port:

- Added `ControllerSessionLedger` in `crates/clankers-controller/src/session_ledger.rs` for session ID lookup, persisted-message checks, active-leaf appends, and compaction summary recording.
- Changed controller persistence and shutdown flushing to use the ledger port instead of `clankers_session::SessionManager`.
- Added root-shell `SessionManagerControllerSessionLedger` adapter in `src/agent_runtime_adapters.rs` and wired standalone/daemon controller construction through it.
- Kept standalone branch/merge slash behavior at the root edge by downcasting the root adapter only inside root event-loop code; reusable controller code no longer imports `clankers-session`.

## Dependency result

`clankers-controller` normal workspace dependencies are now:

```text
["clanker-message", "clankers-agent", "clankers-core", "clankers-protocol"]
```

The controller production internal dependency count in the lego baseline decreased from 5 to 4, and concrete dependencies decreased from 3 to 2.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-controller -p clankers --tests
cargo test -p clankers-controller --lib
cargo test -p clankers --test session_resume_deterministic_replay
cargo test -p clankers-controller --test fcis_shell_boundaries
scripts/check-controller-runtime-boundary.rs
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0.
