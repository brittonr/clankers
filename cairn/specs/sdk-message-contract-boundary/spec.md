# Sdk Message Contract Boundary Specification

## Purpose

Defines how `clanker-message` separates stable embedded SDK contracts from Clankers desktop transcript compatibility records, and how validation prevents transcript IDs, timestamps, and shell history records from leaking into green SDK APIs.

## Requirements

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

### Requirement: Default message SDK subset is transcript-free [r[sdk-message-contract-boundary.default-subset]]

`clanker-message` default SDK imports MUST expose stable content, tool, usage, streaming, result, and semantic-event contracts without requiring Clankers transcript IDs, random ID generation, wall-clock timestamps, bash records, branch summaries, compaction summaries, or custom desktop history records.

#### Scenario: minimal SDK graph excludes transcript dependencies [r[sdk-message-contract-boundary.default-subset.minimal-graph]]
- GIVEN a minimal embedded SDK example depends on `clanker-message` through default SDK entrypoints
- WHEN dependency and source-boundary validation runs
- THEN the default path MUST NOT require transcript compatibility types
- AND it MUST NOT require timestamp or random-ID dependencies unless an explicit compatibility feature is enabled

#### Scenario: stable root exports avoid transcript internals [r[sdk-message-contract-boundary.default-subset.root-exports]]
- GIVEN SDK consumers import stable message contracts from crate-root convenience exports
- WHEN public root exports are inspected
- THEN stable root exports MUST be limited to reusable content, contracts, streaming, result, tool-result, and semantic-event DTOs
- AND transcript compatibility records MUST NOT be re-exported as default stable SDK items

### Requirement: Transcript compatibility is explicit [r[sdk-message-contract-boundary.transcript-compat-feature]]

Clankers transcript compatibility records MAY remain public only through an explicit compatibility module or feature that documents ownership by desktop/session/provider/controller adapters.

#### Scenario: compatibility callers opt in [r[sdk-message-contract-boundary.transcript-compat-feature.opt-in]]
- GIVEN a desktop or adapter path needs `AgentMessage`, `MessageId`, `generate_id`, or persisted transcript variants
- WHEN it imports those records
- THEN the import MUST use the explicit transcript compatibility boundary
- AND the generic embedded SDK examples MUST NOT depend on that compatibility boundary

#### Scenario: compatibility serialization remains covered [r[sdk-message-contract-boundary.transcript-compat-feature.serialization]]
- GIVEN existing Clankers transcript records remain readable for desktop/session adapters
- WHEN compatibility fixtures run
- THEN persisted user, assistant, tool-result, bash, branch, compaction, and custom transcript records MUST deserialize or report an explicit migration error
