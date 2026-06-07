Artifact-Type: implementation-validation-evidence
Task-ID: I1,I2,I3,V1
Covers: remaining-coupling-drain.controller-command-seams.constructor-owners
Status: complete

## Reviewed-Evidence

Projection owner inventory:

- `crates/clankers-controller/src/convert.rs` owns controller domain/semantic event projection.
- `crates/clankers-controller/src/transport_convert.rs` owns daemon control/attach/session wire DTO construction.
- `crates/clankers-provider/src/router_request_bridge.rs` owns provider-router request/cache-key projection from compatibility requests.
- `src/modes/session_command_policy.rs` emits neutral `SessionCommandIntent` / `SessionCommandEffect` data; `src/slash_commands/effects.rs` projects protocol `SessionCommand` values at the slash/attach edge.
- `crates/clankers-controller/tests/fcis_shell_boundaries.rs` now inventories non-test constructor paths and enforces `ControlResponse::*` plus `AttachResponse::*` ownership in `transport_convert.rs`.

Commands run:

```text
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-controller --test fcis_shell_boundaries
Summary [1.181s] 44 tests run: 44 passed, 0 skipped

scripts/check-provider-router-boundary.rs
ok: provider/router boundary rail passed

scripts/check-lego-architecture-boundaries.rs
lego architecture dependency ownership inventory written to target/lego-architecture/dependency-ownership-inventory.json
```

## Decision

The additional DTO family enforced in this drain is daemon control/attach protocol response construction. Reusable controller/daemon bridge logic must go through `transport_convert.rs` for `ControlResponse::*` and `AttachResponse::*` constructors.

## Follow-Up

Extend the same constructor-owner inventory pattern to any future ACP/MCP/RPC DTO families that gain reusable controller logic.
