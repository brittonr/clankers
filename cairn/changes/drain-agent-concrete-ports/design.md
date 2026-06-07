# Design: Drain Agent Concrete Ports

## Context

`target/lego-architecture/dependency-ownership-inventory.json` classifies `clankers-agent` as a turn orchestration shell with eight concrete dependency families still present. Existing ports cover model execution, tool execution, cost, and cancellation, but the crate still imports concrete storage, hook, prompt, skill, procmon, provider, and utility crates.

## Decisions

### 1. Drain by dependency family, not by broad crate move

**Choice:** Treat each concrete dependency as a separate family with an owner, adapter module, DTO boundary, and focused test.

**Rationale:** A blanket extraction risks moving shell state into another reusable crate. Family-level drains let us reduce coupling without hiding desktop policy behind a larger abstraction.

### 2. Reusable turn policy accepts only neutral ports and DTOs

**Choice:** Turn helpers may depend on `clanker-message`, `clankers-core`, engine DTOs, and agent-owned service traits; prompt, skill, storage/search, hook, procmon, provider, and model-selection behavior must be injected by host adapters.

**Rationale:** The agent loop is still orchestration, but reusable turn policy should not know where prompts come from, how search indexes are opened, how hooks are dispatched, or which provider/router implementation backs a model request.

### 3. Provider-native types remain in the model adapter only until collapse

**Choice:** `CompletionRequest` and `Provider` remain allowed only in the declared model adapter modules until a later provider/router convergence slice removes them.

**Rationale:** The provider dependency cannot disappear in one step while the adapter still owns request execution. The near-term rule is to prevent provider-native types from spreading beyond that seam.

### 4. The budget rail is the acceptance oracle

**Choice:** Every drained family must lower or split the concrete dependency budget and update the ownership receipt with source hashes and convergence notes.

**Rationale:** A successful refactor must be measurable. A lower budget prevents future work from silently reintroducing a concrete edge.

## Risks / Trade-offs

- Removing dependencies can expose helper APIs that were only reachable through concrete crates; add narrow DTOs instead of passing whole config/provider/session types.
- Test fixtures may currently construct concrete services directly; keep deterministic fakes near the new port definitions.
- Provider execution remains a deliberate temporary adapter exception until the provider/router boundary is collapsed.
