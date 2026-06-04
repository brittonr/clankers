## ADDED Requirements

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
