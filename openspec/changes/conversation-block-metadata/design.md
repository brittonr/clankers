## Context

Clankers already groups one user prompt plus the resulting assistant and tool activity into conversation blocks. The shared `ConversationBlock` type has a timestamp field, but today that value is assigned from `Local::now()` when the TUI block is created. Restored history rebuilds blocks from persisted messages, so replayed blocks get fresh local timestamps instead of the original time. There is also no stable block identity beyond transient block IDs, which makes block-oriented features replay-sensitive.

This change is cross-cutting because block metadata is assembled in more than one place: live TUI event handling, session restore, controller history replay, and attach-mode rendering all build or rebuild blocks.

## Goals / Non-Goals

**Goals:**
- Give every conversation block one canonical timestamp that survives replay.
- Give every finalized conversation block one canonical BLAKE3 hash derived from stable block content.
- Keep metadata derivation deterministic across live, restored, and attached sessions.
- Avoid session-file migrations when old sessions already contain enough message timestamps to reconstruct block metadata.

**Non-Goals:**
- Replacing message-level timestamps already stored on `AgentMessage` values.
- Adding cryptographic signatures, Merkle trees, or tamper-proof storage.
- Defining a user-facing workflow around block hashes beyond exposing the shared metadata.
- Hashing transient UI state or presentation-only formatting.

## Decisions

### 1. Derive block timestamps from the opening user message

**Choice:** The canonical block timestamp is the timestamp of the user message that starts the block. Internally it should stay in UTC or another canonical representation; rendering can continue converting to local time for display.

**Rationale:** A conversation block starts when the user prompt enters history. That timestamp already exists in persisted messages, so old sessions can reconstruct block times without migration. It also stays stable across restore, attach, and export paths.

**Alternative:** Keep using TUI creation time. Rejected because replay changes the value and makes block metadata depend on when the UI happened to rebuild history.

### 2. Compute a versioned BLAKE3 hash from a canonical block envelope

**Choice:** Finalized blocks hash a canonical envelope that includes the block timestamp plus the ordered persisted message content that belongs to the block. The envelope excludes transient UI-only state such as block IDs, collapse/focus state, scroll position, and local formatting. The hashing helper should carry an explicit schema version so future canonicalization changes can be introduced without silent drift.

Pin v1 now:
- Encode UTF-8 JSON from one pure `CanonicalBlockEnvelopeV1` struct.
- Top-level field order is `v`, `started_at`, `items`.
- `v` is the integer `1`.
- `started_at` is the canonical block timestamp serialized as RFC3339 UTC.
- `items` is the ordered sequence of hashed block items.
- Each item starts with `kind`, followed by kind-specific stable fields in declared struct order.
- Kind-specific fields may include only persisted block content and stable metadata needed to distinguish content, such as tool name, tool input, tool-result error flag, text/thinking payloads, and image payload order.
- Item fields MUST NOT include transient UI state, local block IDs, collapse/focus flags, token counters, scroll offsets, or local-time render formatting.

**Rationale:** BLAKE3 is fast, already present in the workspace, and the project default when no other hash is required. A versioned canonical envelope gives deterministic cross-path hashes and prevents accidental instability from serde field-order or UI-only fields.

**Alternative:** Hash ad hoc formatted strings inside each caller. Rejected because live/replay paths would drift quickly and reviews would have no single contract to test.

### 3. Use one shared block-metadata builder for live and replay paths

**Choice:** Introduce one pure helper that assembles canonical block metadata from the opening user message plus the ordered block contents, then have live block construction, session restore, and controller/attach replay all use that helper.

**Rationale:** Current block assembly is split across `App::start_block`, session restore, and controller replay conversion. A shared helper prevents timestamp drift and hash mismatches between modes.

**Alternative:** Recompute metadata differently in each path. Rejected because the same session would produce different metadata depending on which UI or transport reconstructed it.

### 4. Make hashes final-only, not constantly recomputed UI state

**Choice:** Streaming blocks carry their canonical timestamp immediately, but the finalized BLAKE3 hash becomes authoritative only when the block is complete. Callers may show no hash or a pending state while streaming.

**Rationale:** Tool output and assistant text can still change during streaming. Delaying final publication avoids exposing hashes that immediately become stale.

**Alternative:** Rehash after every delta and treat the latest value as stable. Rejected because it complicates semantics and creates churn for little user value.

## Risks / Trade-offs

- **[Canonicalization drift]** -> Mitigate with one shared helper, schema-versioned hash input, and exact replay fixture tests.
- **[Replay/path skew]** -> Mitigate by routing live, restore, and attach block assembly through the same metadata logic instead of path-local formatting.
- **[Large tool/image payload cost]** -> Mitigate by hashing only on block finalization and by feeding canonical bytes directly to BLAKE3 instead of building multiple redundant string copies.
- **[Old sessions missing enough structure for edge cases]** -> Mitigate by defining the block envelope in terms of persisted messages already present in session history and adding replay coverage over legacy/restored sessions.

## Migration Plan

1. Add canonical timestamp/hash fields to the shared block model.
2. Add the versioned canonical envelope and BLAKE3 helper with fixture tests.
3. Update live TUI block construction to set the canonical timestamp from the opening user message and finalize the hash when the block completes.
4. Update restore and attach/controller replay paths to use the same metadata builder.
5. Validate old sessions by replaying persisted history and confirming timestamps are preserved and hashes are deterministic.

No on-disk migration is required for existing sessions if message timestamps are already available.

## Open Questions

- Should the first user-facing surface for the hash be block details/debug output only, or should the main block header show a short digest?
- Do exported block-oriented formats need the full 32-byte digest, a hex string, or both?
