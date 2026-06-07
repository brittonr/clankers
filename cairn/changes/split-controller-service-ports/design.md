# Design: Split Controller Service Ports

## Context

The controller is transport-agnostic but not dependency-light. Its current ownership receipt permits concrete edges only if each edge is a named adapter seam. Remaining risk clusters are provider thinking compatibility in command policy, persistence/search access in controller persistence, hook dispatch in event processing, and protocol projection drift.

## Decisions

### 1. Treat the controller as an orchestration shell with service ports

**Choice:** Keep the controller responsible for sequencing session commands and effects, but require external behavior to flow through explicit runtime, persistence, hook, and projection ports.

**Rationale:** The controller should coordinate behavior, not own provider request shaping, storage schema details, or transport DTO construction.

### 2. Runtime execution owns agent/provider details

**Choice:** `ControllerRuntimeAdapter` or its successor owns prompt/control execution against the agent/runtime. Command policy should not import provider-native thinking/request types once the adapter can translate neutral controller intents.

**Rationale:** Provider compatibility in command handling makes request-shape changes leak across controller policy and undermines FCIS boundaries.

### 3. Persistence becomes a session service port

**Choice:** Session store/search operations used by controller logic move behind a typed session persistence service. `persistence.rs` may adapt to `clankers-session` and `clankers-db`, but command and event modules should consume neutral results.

**Rationale:** Storage format and search indexing are shell services. Controller policy should not know whether JSONL, Automerge, redb, or another store backs a session.

### 4. Projection owners remain constructor-only edges

**Choice:** `convert.rs` and `transport_convert.rs` remain the only normal owners for daemon/protocol output constructors; command modules emit neutral domain outputs.

**Rationale:** Existing FCIS rails already enforce constructor ownership. This change tightens the service seams feeding those projection owners rather than moving projection policy.

## Risks / Trade-offs

- Port extraction may add indirection around tests; keep deterministic fake services small and next to the port traits.
- Moving provider-thinking compatibility can break slash/resume behavior unless runtime-path tests capture request metadata.
- Persistence port changes must preserve replay/history ordering and Automerge migration behavior.
