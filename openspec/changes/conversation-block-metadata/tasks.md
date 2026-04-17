## 1. Canonical block metadata model

- [ ] 1.1 Add shared block metadata fields for canonical timestamp and finalized BLAKE3 hash in the conversation block model, then grep for `ConversationBlock {` and other helper constructors and update every construction site
- [ ] 1.2 Add a versioned canonical block-envelope helper that excludes transient UI-only state from hashing
- [ ] 1.3 Pin the v1 canonical envelope in code comments or fixtures so field order and included fields stay reviewable
- [ ] 1.4 Add unit tests that prove identical canonical block content yields the same BLAKE3 hash and transient UI state does not affect it

## 2. Live block construction

- [ ] 2.1 Update live block start/finalize paths to take the opening user-message timestamp instead of `Local::now()` as the canonical block timestamp
- [ ] 2.2 Finalize the BLAKE3 hash only after the block's assistant/tool content is complete, leaving in-progress blocks without a published final hash
- [ ] 2.3 Add live-path tests covering normal turns, tool-heavy turns, and branched turns so timestamps stay stable and distinct per block

## 3. Restore and replay parity

- [ ] 3.1 Update session-restore block reconstruction to preserve the original block timestamp from persisted messages
- [ ] 3.2 Update controller/attach replay paths to derive the same block timestamp and finalized BLAKE3 hash as standalone mode
- [ ] 3.3 Add replay fixtures or integration tests that restore the same persisted session through standalone and attach paths and assert metadata parity

## 4. Validation and surfaces

- [ ] 4.1 Update block-oriented rendering or diagnostics surfaces to consume the canonical timestamp field without reintroducing wall-clock rebuild time
- [ ] 4.2 Add regression coverage proving that changing prompt text, assistant/tool content, or canonical timestamp changes the finalized hash
- [ ] 4.3 Add regression coverage proving that restored history no longer gets fresh timestamps after replay
