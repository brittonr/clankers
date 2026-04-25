Evidence-ID: decouple-llm-contract-surface.validation-plan
Task-ID: V1,V2,V3,V4,V5
Artifact-Type: validation-log
Covers: embeddable-agent-engine.adapter-transcript-conversion, embeddable-agent-engine.no-agent-message-filtering, embeddable-agent-engine.engine-cargo-tree-clean, embeddable-agent-engine.message-without-router, embeddable-agent-engine.cargo-tree-rail, embeddable-agent-engine.source-surface-rail, embeddable-agent-engine.contract-boundary-rails, embeddable-agent-engine.router-provider-reexports
Creator: pi
Created: 2026-04-25T00:00:00Z
Status: OPEN

## Purpose

Durable validation log for `decouple-llm-contract-surface`. Each V task appends command output excerpts here before its checkbox is marked done.

## Required Sections

- `V1` post-migration re-export/type-identity tests.
- `V2` adapter transcript conversion tests.
- `V3` normal-edge cargo-tree boundary rail output.
- `V4` source-inventory rail output.
- `V5` focused final compatibility and adapter test output.

## Evidence Log

### V1 — post-migration re-export/type-identity tests

Command:

```bash
cargo test -p clankers-provider --lib tests::router_and_provider_contract_paths_resolve_to_message_types && \
  cargo test -p clankers-provider --lib tests::router_and_provider_do_not_define_independent_stream_delta
```

Result: PASS (`pueue` task 130)

Output excerpt:

```text
test tests::router_and_provider_contract_paths_resolve_to_message_types ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 166 filtered out; finished in 0.00s

test tests::router_and_provider_do_not_define_independent_stream_delta ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 166 filtered out; finished in 0.00s
```

### V2 — pending

No command output captured yet.

### V3 — pending

No command output captured yet.

### V4 — pending

No command output captured yet.

### V5 — pending

No command output captured yet.
