## ADDED Requirements

### Requirement: Each conversation block has a canonical timestamp
The system MUST assign every conversation block a canonical timestamp derived from persisted conversation history, not from transient TUI construction time.

#### Scenario: Live block captures its opening timestamp
- **WHEN** a user starts a new conversation block
- **THEN** the block timestamp MUST be set from the opening user message timestamp
- **AND** the timestamp MUST remain unchanged while assistant text, tool calls, and tool results stream into that block

#### Scenario: Restored history preserves original block time
- **WHEN** an existing session is restored, attached, or replayed from persisted messages
- **THEN** each reconstructed block MUST carry the same timestamp it had when first recorded
- **AND** the implementation MUST NOT replace it with the current wall-clock time

#### Scenario: Branched blocks get distinct timestamps
- **WHEN** a user branches from an earlier block and creates a new prompt
- **THEN** the new block MUST receive its own opening timestamp from that new user message
- **AND** the parent block timestamp MUST remain unchanged

### Requirement: Each finalized conversation block has a canonical BLAKE3 hash
The system MUST compute a finalized BLAKE3 hash for each completed conversation block from a canonical representation of the block's persisted content and timestamp.

#### Scenario: Replay yields the same hash
- **WHEN** the same finalized block is reconstructed from the same persisted message sequence
- **THEN** the reconstructed block MUST have the same BLAKE3 hash as the original finalized block

#### Scenario: Content changes change the hash
- **WHEN** any hashed part of a block changes, including prompt text, assistant text, tool-call content, tool-result content, or canonical timestamp
- **THEN** the finalized BLAKE3 hash for that block MUST change

#### Scenario: Ephemeral UI state does not affect the hash
- **WHEN** only transient block state changes, such as local block ID, collapse state, focus state, scroll state, streaming state, or rendered timezone formatting
- **THEN** the finalized BLAKE3 hash MUST remain unchanged

### Requirement: Shared block surfaces expose consistent metadata
The system MUST expose the same canonical block timestamp and finalized BLAKE3 hash through the shared block model used by live rendering, history replay, and machine-readable block consumers.

#### Scenario: Standalone and attach mode agree on metadata
- **WHEN** standalone mode and attach mode render the same persisted session history
- **THEN** each visible conversation block MUST show the same canonical timestamp and finalized BLAKE3 hash in both modes

#### Scenario: In-progress blocks defer final hash publication
- **WHEN** a block is still streaming and its full persisted content is not final yet
- **THEN** the block MAY omit the finalized BLAKE3 hash while streaming
- **AND** it MUST publish the finalized hash once the block is complete
