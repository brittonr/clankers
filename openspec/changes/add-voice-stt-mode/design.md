## Context

This change tracks Hermes feature-parity work for Voice and Speech-to-Text Mode. Clankers already has strong Rust-native agent, daemon, plugin, routing, scheduling, and tool foundations; this change should compose with those foundations rather than bypass them.

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

**Implementation:** Add the minimum new module/crate surface needed for Voice and Speech-to-Text Mode, then wire it through the existing CLI/TUI/daemon paths.

### 2. Make policy and observability first-class

**Choice:** Every implementation MUST expose enough state for tests, logs, session replay, and user-facing errors.

**Rationale:** These features often cross process, network, or file boundaries. Silent fallback is harder to debug than a clear unsupported-path error.

**Alternative:** Optimize only for a happy-path demo. Rejected because these are agent autonomy features and failures must be recoverable.

## Risks / Trade-offs

**Scope creep** → Start with a minimal backend/API and document additional backends as future tasks.

**Security regressions** → Reuse sanitized environments, capability checks, and explicit allowlists.

**Session replay drift** → Store normalized events/metadata rather than backend-specific blobs when possible.
