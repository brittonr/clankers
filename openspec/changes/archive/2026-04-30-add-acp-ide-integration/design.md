## Context

This change tracks Hermes feature-parity work for ACP IDE Integration. Clankers already has strong Rust-native agent, daemon, plugin, routing, scheduling, and tool foundations; this change should compose with those foundations rather than bypass them.

## Goals / Non-Goals

**Goals:**
- Provide a small, testable first implementation that is useful from the TUI, prompt mode, and daemon/session paths.
- Keep policy decisions explicit: credentials, sandboxing, persistence, and output delivery must be auditable.
- Document gaps intentionally left for follow-up.

**Non-Goals:**
- Large rewrites of the agent loop or provider stack unless required by the capability boundary.
- Hidden best-effort behavior that silently drops outputs, credentials, or session context.

## Decisions

### 1. Build on existing clankers primitives

**Choice:** Reuse existing tool registration, daemon/session persistence, config paths, provider routing, and plugin/runtime abstractions where possible.

**Rationale:** This keeps the feature consistent with clankers architecture and avoids Hermes-shaped islands that are hard to maintain.

**Alternative:** Copy Hermes behavior directly as a separate subsystem. Rejected because duplicated lifecycle and policy handling would drift quickly.

**Implementation:** Add the minimum new module/crate surface needed for ACP IDE Integration, then wire it through the existing CLI/TUI/daemon paths.

### 2. Make policy and observability first-class

**Choice:** Every implementation MUST expose enough state for tests, logs, session replay, and user-facing errors.

**Rationale:** These features often cross process, network, or file boundaries. Silent fallback is harder to debug than a clear unsupported-path error.

**Alternative:** Optimize only for a happy-path demo. Rejected because these are agent autonomy features and failures must be recoverable.

### 3. Start with a foreground stdio ACP adapter

**Choice:** The first user-facing surface is `clankers acp serve`, a foreground stdio adapter for one clankers session. ACP remains an editor transport, not a model-callable built-in tool.

**Rationale:** ACP-compatible editors can launch foreground stdio commands, and clankers already owns session construction, prompt dispatch, tool policy, and persistence. Stdio keeps the first pass local and avoids network listener policy before the protocol seam is proven.

**Alternative:** Add a background daemon listener or a durable config section first. Rejected for the first pass because listener lifecycle, authentication, and multi-session routing would expand the security surface before the adapter is tested.

### 4. Return explicit unsupported ACP errors

**Choice:** Methods outside the first supported prompt/session subset MUST return structured unsupported errors rather than silently succeeding or dropping editor requests.

**Rationale:** IDE integrations are hard to debug when terminal, diff, cancellation, or media operations are ignored. Explicit errors preserve user trust and make follow-up OpenSpec slices easier to define.

## Risks / Trade-offs

**Scope creep** → Start with a minimal backend/API and document additional backends as future tasks.

**Security regressions** → Reuse sanitized environments, capability checks, and explicit allowlists.

**Session replay drift** → Store normalized events/metadata rather than backend-specific blobs when possible.
