## Context

The current architecture keeps engine/tool-host SDK crates generic and lets Clankers-specific shells compose prompts, stores, confirmation policy, tools, provider adapters, and events. The risk is that future shell fixes reintroduce duplicated turn policy or skip translation in one shell path.

## Goals / Non-Goals

**Goals:** test representative shell entrypoints over a shared matrix of runtime features and assert adapter-only ownership.

**Non-Goals:** full TUI visual regression, live Matrix/iroh network tests, or live provider credentials.

## Decisions

### 1. Shared transcript fixtures across shells

**Choice:** define recorded prompt/tool/model fixtures that can be driven through standalone agent, controller/daemon adapter seams, and bounded batch/embedded paths.

**Rationale:** shared fixtures make shell drift visible without depending on live providers.

### 2. Feature axes mirror host-owned services

**Choice:** include prompt source, store mode, confirmation response, disabled-tool policy, tool result class, model result class, and event translation as axes.

**Rationale:** these are the seams where app policy should remain outside the engine but consistent across shells.

### 3. FCIS remains the policy guardrail

**Choice:** extend FCIS/source-boundary tests with matrix execution evidence rather than replacing them.

**Rationale:** source-boundary checks catch architectural regression; matrix fixtures catch behavioral drift.
