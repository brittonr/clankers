Artifact-Type: implementation-validation-evidence
Task-ID: I1,I2,I3,V1
Covers: remaining-coupling-drain.root-shell-policy.root-module-ownership-map, remaining-coupling-drain.root-shell-policy.policy-slice-drain
Status: complete

## Reviewed-Evidence

Root ownership inventory:

- `policy/lego-architecture/dependency-ownership-baseline.json` records the root `clankers` crate as product-shell wiring with 29 internal dependencies and per-edge owner receipts.
- Root shell buckets represented in the baseline include:
  - `root_crate.owner_receipts` for shell-wiring and edge-projection dependencies;
  - `session_command_policy` for neutral session command effects and protocol projection at `src/slash_commands/effects.rs`;
  - `process_tool_adapter` for the root process tool as JSON/request projection over typed backend services;
  - `daemon_session_assembly` for socketless session runtime assembly outside the actor loop.
- Selected policy slice: session command policy is drained into neutral `SessionCommandIntent`, `SessionCommandEffect`, `SessionAckPolicy`, and `LocalSessionEffect` in `src/modes/session_command_policy.rs`; protocol constructors are projected only at `src/slash_commands/effects.rs` and attach adapters.

Commands run:

```text
scripts/check-lego-architecture-boundaries.rs
lego architecture dependency ownership inventory written to target/lego-architecture/dependency-ownership-inventory.json

env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-controller --test fcis_shell_boundaries
Summary [1.181s] 44 tests run: 44 passed, 0 skipped
```

## Decision

Root remains an application-edge shell. The drained root slice uses neutral session command policy DTOs and explicit protocol projection owners, while the architecture baseline keeps remaining root dependency edges accountable with convergence conditions.

## Follow-Up

Future root drains should reduce one temporary-policy row at a time and update the lego architecture baseline instead of adding reusable behavior directly under root `src/`.
