## Context

Clankers already has a canonical `browser-automation` specification or adjacent Hermes-parity baseline. This change narrows the next implementation slice so future work can proceed without re-scoping the feature.

## Goals / Non-Goals

**Goals:**
- Preserve the daemon/session/tool policy boundaries already used by clankers.
- Match the useful Hermes behavior while keeping metadata safe and deterministic.
- Keep implementation slices testable without live credentials or external services by default.

**Non-Goals:**
- Do not bypass existing confirmation, capability, disabled-tool, or session persistence paths.
- Do not make live network/cloud/provider behavior mandatory for deterministic CI.
- Do not persist raw prompts, secrets, audio, browser scripts, credentials, or platform destinations in replay/debug metadata.

## Decisions

### 1. Build on the existing domain seam

**Choice:** Extend the existing `browser-automation` capability rather than creating a parallel feature surface.

**Rationale:** The prior Hermes parity baselines already established ownership and safety boundaries. Follow-up work should deepen those seams instead of fragmenting command/tool behavior.

**Alternative:** Add a new top-level tool or mode for this slice. Rejected because it would duplicate policy, documentation, and tests.

**Implementation:** Add minimal typed models, helpers, and adapters in the existing owning modules, then wire through shared TUI/daemon/session construction only after deterministic tests exist.

### 2. Safe receipts before broad live behavior

**Choice:** Every side-effecting or external operation must produce normalized receipts and redacted metadata before broad live integrations are considered complete.

**Rationale:** Hermes parity is valuable only if clankers keeps stronger auditability and capability boundaries.

**Alternative:** First ship live behavior and add receipts later. Rejected because it makes regressions hard to diagnose and can leak sensitive data.

**Implementation:** Define receipt structs/tests with representative success and failure cases, assert secret-like fields are absent, and persist only safe identifiers, statuses, counts, hashes, handles, and error classes.

## Risks / Trade-offs

**Scope creep** → Keep the tasks ordered so policy/model tests land before runtime breadth.

**Live environment flakiness** → Use fake runtimes and self-skipping smoke tests for optional live coverage.

**Mode drift** → Route through shared standalone/TUI/daemon/session helpers rather than mode-specific ad hoc paths.
