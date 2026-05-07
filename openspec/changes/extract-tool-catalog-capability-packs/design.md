## Context

Tool registration is central to embedding risk. A host app may want no shell access, read-only file tools, a private app toolset, or full Clankers defaults. A reusable builder makes this explicit instead of forcing hosts through `src/modes/common.rs`.

## Decisions

### Capability packs as explicit publication policy

**Choice:** Group tools into named packs with safe defaults rather than publishing the full Clankers tool surface by default.

**Rationale:** Embedders can reason about risk at a product-policy level. Dangerous machine-control tools become opt-in.

**Rejected:** A single boolean such as `enable_tools`. It hides important differences between read-only tools, mutating filesystem tools, shell/process execution, browser automation, and plugin/MCP process startup.

### Builder owns catalog shape, tools own execution

**Choice:** The builder selects and configures tools; individual tool modules retain execution logic.

**Rationale:** Extraction should reduce wiring coupling without rewriting every tool.

### Host custom tools are first-class

**Choice:** The builder must accept host-provided tools and collision policy.

**Rationale:** Embedding is valuable when an app exposes its own domain actions to the agent while selectively using Clankers built-ins.

## Risks / Trade-offs

- **Policy drift:** Existing TUI/daemon defaults may diverge from builder defaults. Mitigate with publication parity tests.
- **Overbroad packs:** Pack names must remain conservative and documented; shell/process/plugin/MCP packs are never implicitly enabled for embedded safe defaults.
- **Schema-cost regressions:** Builder should preserve existing tiering/filtering so hosts can constrain tool prompt cost.
