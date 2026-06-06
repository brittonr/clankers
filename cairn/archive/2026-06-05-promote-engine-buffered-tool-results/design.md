# Design: Promote Engine Buffered Tool Results

## Context

`EngineState::buffered_tool_results` is already documented as a supported public field because embedders can observe the engine while it waits for all parallel tool calls. The element type, `EngineBufferedToolResult`, remained experimental from the first budget pass even though the reducer now has deterministic coverage for buffering order, duplicate feedback rejection, and clearing the buffer before the follow-up model request.

## Decisions

### 1. Promote the existing DTO rather than hiding the buffer

**Choice:** Mark `EngineBufferedToolResult` and its fields as `supported`.

**Rationale:** Hiding the buffer would make `EngineState` partially opaque and break struct-literal construction for an otherwise supported state type. Promoting the element type keeps the public state contract internally consistent while preserving the existing reducer-owned shape.

### 2. Keep validation at the reducer owner

**Choice:** Use `cargo test -p clankers-engine --lib tool_feedback` and the SDK budget rails as the validation path.

**Rationale:** The buffer is reducer state, not a desktop adapter feature. Owner tests should prove that partial feedback is buffered, duplicate feedback fails closed, and completed feedback is ordered into follow-up messages before any aggregate SDK rail claims promotion.

## Risks / Trade-offs

- Stabilizing the DTO means future shape changes need migration notes; this is acceptable because the public `EngineState` field already exposed the shape.
- The change reduces the experimental budget but does not touch the larger `clanker-message` transcript compatibility group; that remains for a separate slice.
