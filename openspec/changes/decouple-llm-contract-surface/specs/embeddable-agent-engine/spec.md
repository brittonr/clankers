## ADDED Requirements

### Requirement: Engine contract dependencies MUST remain embeddable

The engine contract surface MUST depend only on reusable plain-data crates and MUST NOT require provider, router, daemon, UI, network, database, or async-runtime implementation crates to compile.
r[embeddable-agent-engine.minimal-contract-dependencies]

#### Scenario: engine cargo tree excludes runtime provider graph
r[embeddable-agent-engine.engine-cargo-tree-clean]
- **WHEN** validation inspects normal dependencies for `clankers-engine`
- **THEN** the dependency graph does not include `clankers-provider`, `clanker-router`, `tokio`, `reqwest`, `redb`, `iroh`, `ratatui`, `crossterm`, `portable-pty`, or `clankers-agent`
- **THEN** failure blocks acceptance of this change

#### Scenario: message contracts do not depend on router runtime
r[embeddable-agent-engine.message-without-router]
- **WHEN** validation inspects normal dependencies for `clanker-message`
- **THEN** the dependency graph does not include `clanker-router` or router-only runtime dependencies
- **THEN** generic message, content, tool, thinking, usage, and stream contract types remain available from `clanker-message`

#### Scenario: router and provider consume canonical message contracts
r[embeddable-agent-engine.router-provider-reexports]
- **WHEN** router or provider code exposes LLM contract types used by existing Clankers call sites
- **THEN** those types are imported from or re-exported from the canonical `clanker-message` definitions
- **THEN** no independent duplicate `Usage`, `ToolDefinition`, `ThinkingConfig`, or stream-delta type identity is introduced

### Requirement: Engine prompt submission MUST use engine-native transcripts

The engine prompt submission API MUST accept engine-native transcript data rather than Clankers shell message enums.
r[embeddable-agent-engine.engine-native-submission]

#### Scenario: engine no longer filters shell message variants
r[embeddable-agent-engine.no-agent-message-filtering]
- **WHEN** a host submits conversation context to the engine
- **THEN** the submitted messages are already canonical `EngineMessage` values
- **THEN** the engine does not depend on `AgentMessage` or decide how to drop Clankers-specific `BashExecution`, `Custom`, `BranchSummary`, or `CompactionSummary` messages

#### Scenario: Clankers adapter owns transcript conversion
r[embeddable-agent-engine.adapter-transcript-conversion]
- **WHEN** the Clankers agent runtime invokes the engine with its persisted conversation history
- **THEN** adapter code converts shell-native `AgentMessage` values into `EngineMessage` values before calling the engine
- **THEN** positive and negative tests cover included user/assistant/tool messages and excluded shell-only message variants

### Requirement: Boundary rails MUST prevent contract dependency regressions

The repository MUST provide deterministic validation rails that fail if the embeddable engine contract regains runtime or shell-only dependencies.
r[embeddable-agent-engine.contract-boundary-rails]

#### Scenario: cargo-tree rail rejects forbidden transitive crates
r[embeddable-agent-engine.cargo-tree-rail]
- **WHEN** the embeddable-engine validation bundle runs
- **THEN** it checks `cargo tree` output for `clankers-engine` and `clanker-message`
- **THEN** forbidden provider/router/runtime crates cause a clear failure message

#### Scenario: source rail rejects forbidden public surface imports
r[embeddable-agent-engine.source-surface-rail]
- **WHEN** the FCIS-style boundary test inventories non-test engine and message contract source
- **THEN** it fails on provider-shaped `CompletionRequest`, daemon protocol types, TUI types, Tokio handles, timestamps, shell-generated message IDs, shell request construction, or `AgentMessage` use in the engine public input surface
- **THEN** it allows adapter-only conversion code outside `clankers-engine`
