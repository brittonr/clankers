## Context

`tool-gateway-platform-delivery` currently centralizes toolset policy and provides `deliver-receipt` as a safe metadata-only surface. This is correct for first-pass safety but not enough for production platform delivery: there is no adapter call, retry state, platform context binding, or artifact handoff path.

## Goals / Non-Goals

**Goals:**
- Add a real delivery adapter boundary without bypassing the existing Tool Gateway policy.
- Deliver local/session artifacts and Matrix artifacts only when the platform context is explicit and authenticated by the running session/bridge.
- Persist a bounded outbox record per attempt so delivery is auditable and retryable.
- Preserve receipt redaction invariants.

**Non-Goals:**
- Generic webhook/cloud delivery without a separate credential policy.
- Sending arbitrary raw tool payloads; delivery is for declared artifact handles only.
- Background daemon replacement for all platform bridges.

## Decisions

### 1. Gateway owns delivery policy; adapters only execute approved attempts

**Choice:** Build delivery requests through `tool_gateway` policy functions before any adapter sees them.

**Rationale:** This preserves the current shared policy seam and prevents standalone, daemon, or tool code from inventing platform-specific bypasses.

**Alternative:** Let each platform bridge implement its own policy. Rejected because receipt and redaction behavior would drift.

**Implementation:** Introduce typed delivery request/attempt/outbox models in or near `src/tool_gateway.rs`, then call adapter implementations only after target, artifact kind, size/path, and session context pass validation.

### 2. Matrix is the first live platform target

**Choice:** Implement Matrix delivery before webhook/cloud targets.

**Rationale:** Clankers already has Matrix bridge/session concepts, so it can bind delivery to an existing authenticated session rather than accepting raw destinations or new credentials.

**Alternative:** Add webhooks first. Rejected because webhook URLs are high-risk secret-bearing destinations and need a separate credential/store policy.

### 3. Delivery receipts remain safe replay metadata

**Choice:** Receipts include stable attempt id, target kind, status, artifact kind, safe artifact label, optional platform handle, error class, retryability, and hashes/counts where useful.

**Rationale:** Operators need debugging evidence without leaking destinations or artifact contents.

**Alternative:** Store raw payloads/destinations in receipts. Rejected for replay/session safety.

## Risks / Trade-offs

**Duplicate sends** → Use idempotency keys/attempt ids and outbox state transitions before retry.

**Matrix context ambiguity** → Require an active Matrix session/bridge binding; reject user-supplied room IDs as raw destinations in this change.

**Secret leakage** → Add negative tests with marker strings for URLs, tokens, headers, full paths, and payload bytes.
