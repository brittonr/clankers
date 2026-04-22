## ADDED Requirements

### Requirement: Each conversation block has a canonical timestamp
ID: conversation.block.metadata.canonical.timestamp
The system MUST assign every conversation block a canonical timestamp derived from persisted conversation history, not from transient TUI construction time.

#### Scenario: Live block captures its opening timestamp
ID: conversation.block.metadata.canonical.timestamp.live-block-opening-message-time
- **WHEN** a user starts a new conversation block
- **THEN** the block timestamp MUST be set from the opening user message timestamp
- **AND** the timestamp MUST remain unchanged while assistant text, tool calls, and tool results stream into that block

#### Scenario: Restored history preserves original block time
ID: conversation.block.metadata.canonical.timestamp.restore-preserves-original-time
- **WHEN** an existing session is restored, attached, or replayed from persisted messages
- **THEN** each reconstructed block MUST carry the same timestamp it had when first recorded
- **AND** the implementation MUST NOT replace it with the current wall-clock time

#### Scenario: Branched blocks get distinct timestamps
ID: conversation.block.metadata.canonical.timestamp.branch-gets-distinct-time
- **WHEN** a user branches from an earlier block and creates a new prompt
- **THEN** the new block MUST receive its own opening timestamp from that new user message
- **AND** the parent block timestamp MUST remain unchanged

### Requirement: Each finalized conversation block has a canonical BLAKE3 hash
ID: conversation.block.metadata.finalized.blake3.hash
The system MUST compute a finalized BLAKE3 hash for each completed conversation block from a canonical representation of the block's persisted content and timestamp.

#### Scenario: Replay yields the same hash
ID: conversation.block.metadata.finalized.blake3.hash.replay-of-same-sequence-matches
- **WHEN** the same finalized block is reconstructed from the same persisted message sequence
- **THEN** the reconstructed block MUST have the same BLAKE3 hash as the original finalized block

#### Scenario: Content changes change the hash
ID: conversation.block.metadata.finalized.blake3.hash.content-changes-change-digest
- **WHEN** any hashed part of a block changes, including prompt text, assistant text, tool-call content, tool-result content, or canonical timestamp
- **THEN** the finalized BLAKE3 hash for that block MUST change

#### Scenario: Ephemeral UI state does not affect the hash
ID: conversation.block.metadata.finalized.blake3.hash.ui-state-does-not-change-digest
- **WHEN** only transient block state changes, such as local block ID, collapse state, focus state, scroll state, streaming state, or rendered timezone formatting
- **THEN** the finalized BLAKE3 hash MUST remain unchanged

### Requirement: Shared block surfaces expose consistent metadata
ID: conversation.block.metadata.shared.surfaces
The system MUST expose the same canonical block timestamp and finalized BLAKE3 hash through the shared block model used by live rendering, history replay, and machine-readable block consumers.

#### Scenario: Machine-readable block consumers share canonical fields
ID: conversation.block.metadata.shared.surfaces.machine-readable-consumers-share-fields
- **WHEN** code reads a `ConversationBlock` value from live, restored, or replayed history
- **THEN** the shared block model MUST expose the canonical timestamp and finalized BLAKE3 hash on that value
- **AND** machine-readable consumers MUST NOT need wall-clock reconstruction or caller-local rehashing to observe them

#### Scenario: Standalone and attach mode agree on metadata
ID: conversation.block.metadata.shared.surfaces.standalone-and-attach-agree
- **WHEN** standalone mode and attach mode reconstruct the same persisted session history
- **THEN** each shared `ConversationBlock` value MUST expose the same canonical timestamp and finalized BLAKE3 hash in both modes

#### Scenario: In-progress blocks defer final hash publication
ID: conversation.block.metadata.shared.surfaces.streaming-blocks-defer-final-hash
- **WHEN** a block is still streaming and its full persisted content is not final yet
- **THEN** the block MAY omit the finalized BLAKE3 hash while streaming
- **AND** it MUST publish the finalized hash once the block is complete
