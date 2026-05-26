## Context

The change updates the embedded prompt lifecycle so pending prompt state is seeded from `AgentEvent::BeforeAgentStart` and finished through the reducer-owned prompt slot.

## Decisions

Prompt lifecycle changes must preserve prompt traceability across normal and embedded prompt paths.
