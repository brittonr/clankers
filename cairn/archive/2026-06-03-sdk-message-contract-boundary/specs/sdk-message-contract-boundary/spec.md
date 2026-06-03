## ADDED Requirements

### Requirement: Message crate public API is classified [r[sdk-message-contract-boundary.inventory]]

`clanker-message` MUST classify public message, content, tool, usage, streaming, semantic-event, ID, timestamp, and transcript types as stable SDK contract, optional support, compatibility/internal, or experimental.

#### Scenario: inventory labels every public type [r[sdk-message-contract-boundary.inventory.labels]]
- GIVEN generated SDK API inventory scans `clanker-message`
- WHEN a public type or function is listed
- THEN it MUST have a support label and source owner
- AND transcript-internal labels MUST be distinguishable from stable SDK contracts

### Requirement: Stable SDK subset excludes shell transcript internals [r[sdk-message-contract-boundary.stable-subset]]

The stable SDK message subset MUST include reusable content/tool/usage/streaming/semantic-event contracts and MUST NOT require Clankers transcript IDs, timestamps, bash records, branch summaries, compaction summaries, or custom desktop history records.

#### Scenario: stable contracts are documented [r[sdk-message-contract-boundary.stable-subset.contracts]]
- GIVEN SDK documentation names message entrypoints
- WHEN the generated inventory verifies them
- THEN stable contracts MUST include content blocks, tool definitions/results, usage, stop reasons, thinking config, streaming deltas, and semantic events
- AND transcript internals MUST be marked unsupported, experimental, or compatibility-only unless separately promoted

### Requirement: Transcript internals are edge-owned [r[sdk-message-contract-boundary.transcript-internals]]

Clankers-specific transcript records MUST be owned by session/provider/controller compatibility adapters or a dedicated transcript module, not by generic SDK APIs.

#### Scenario: transcript internals are compatibility-only [r[sdk-message-contract-boundary.transcript-internals.compatibility-only]]
- GIVEN `AgentMessage`, message IDs, bash execution, branch summary, compaction summary, or custom messages remain public
- WHEN SDK inventory and docs are reviewed
- THEN those items MUST be labeled compatibility/internal with migration notes
- AND they MUST NOT be required by minimal engine-host examples

#### Scenario: adapter boundaries own transcript conversion [r[sdk-message-contract-boundary.transcript-internals.edge-owned]]
- GIVEN provider, controller, session, or root restore paths need Clankers transcript records
- WHEN conversion crosses into reusable SDK logic
- THEN the adapter MUST convert to stable message/semantic DTOs first or carry an owner receipt

### Requirement: Message contract split is verified [r[sdk-message-contract-boundary.verification]]

Message contract changes MUST preserve existing serialization while preventing transcript internals from leaking into green SDK APIs.

#### Scenario: compatibility fixtures preserve transcript serialization [r[sdk-message-contract-boundary.verification.compat-fixtures]]
- GIVEN existing session/provider/controller paths deserialize Clankers transcript records
- WHEN compatibility fixtures run
- THEN serialized `AgentMessage` and transcript-internal records MUST remain readable or have explicit migration adapters

#### Scenario: boundary rails reject green API leakage [r[sdk-message-contract-boundary.verification.boundary-rails]]
- GIVEN a green SDK API exposes `AgentMessage`, shell transcript variants, generated IDs, or wall-clock timestamps
- WHEN validation runs
- THEN the rail MUST fail unless the exposure is an explicitly documented compatibility adapter
