# memory-tool Specification

## Purpose
TBD - created by archiving change agent-learning-loop. Update Purpose after archive.
## Requirements
### Requirement: Agent can save memory entries

The agent SHALL be able to save a new memory entry by calling the `memory` tool with action `add`. The entry includes text content, a scope (global or project-scoped), and optional tags. The tool SHALL return the entry ID, current char usage, and char limit.

#### Scenario: Save a global memory
- **WHEN** the agent calls `memory` with `{"action": "add", "text": "User prefers snake_case", "scope": "global"}`
- **THEN** the tool persists the entry via `MemoryStore::save` and returns `{"id": <id>, "usage": "245/2200", "status": "saved"}`

#### Scenario: Save a project-scoped memory
- **WHEN** the agent calls `memory` with `{"action": "add", "text": "This repo uses sqlx", "scope": "project"}`
- **THEN** the tool persists the entry with `MemoryScope::Project` using the current working directory as the path

#### Scenario: Reject when at capacity
- **WHEN** the agent calls `memory` with action `add` and the new entry would exceed the configured char limit
- **THEN** the tool returns an error with current entries, usage stats, and a message to consolidate or remove entries first

### Requirement: Agent can replace memory entries

The agent SHALL be able to replace an existing memory entry by calling the `memory` tool with action `replace`. Matching uses substring search on existing entry text.

#### Scenario: Replace by substring
- **WHEN** the agent calls `memory` with `{"action": "replace", "old_text": "snake_case", "text": "User prefers camelCase in TypeScript, snake_case in Rust"}`
- **THEN** the tool finds the unique entry containing "snake_case", updates its text, and returns the updated entry with usage stats

#### Scenario: Ambiguous substring match
- **WHEN** the agent calls `memory` with action `replace` and `old_text` matches multiple entries
- **THEN** the tool returns an error listing the matching entries and asks for a more specific substring

### Requirement: Agent can remove memory entries

The agent SHALL be able to remove a memory entry by calling the `memory` tool with action `remove`. Matching uses substring search.

#### Scenario: Remove by substring
- **WHEN** the agent calls `memory` with `{"action": "remove", "old_text": "snake_case"}`
- **THEN** the tool finds the unique matching entry, removes it, and returns success with updated usage stats

#### Scenario: No match found
- **WHEN** the agent calls `memory` with action `remove` and `old_text` matches no entries
- **THEN** the tool returns an error indicating no matching entry was found

### Requirement: Agent can search memory entries

The agent SHALL be able to search memories by calling the `memory` tool with action `search`. This returns matching entries without modifying anything.

#### Scenario: Search with results
- **WHEN** the agent calls `memory` with `{"action": "search", "query": "database"}`
- **THEN** the tool returns all entries containing "database" (case-insensitive) in text or tags

#### Scenario: Search with no results
- **WHEN** the agent calls `memory` with action `search` and the query matches nothing
- **THEN** the tool returns an empty list

### Requirement: Memory capacity is bounded

The memory store SHALL enforce a configurable character limit (default: 2200 chars for global, 1375 chars for project-scoped). The limit applies to the sum of all entry text lengths within each scope.

#### Scenario: Capacity reported in tool responses
- **WHEN** the agent calls any `memory` tool action that mutates state
- **THEN** the response includes `"usage": "<current>/<limit>"` for the affected scope

#### Scenario: Capacity visible in system prompt
- **WHEN** memory entries are injected into the system prompt at session start
- **THEN** the memory section header includes usage percentage and char counts (e.g., `MEMORY [67% — 1474/2200 chars]`)

### Requirement: Memory capacity is configurable

The memory char limits SHALL be configurable via `settings.yaml` under `memory.global_char_limit` and `memory.project_char_limit`.

#### Scenario: Custom limits
- **WHEN** `settings.yaml` contains `memory: { global_char_limit: 4000 }`
- **THEN** the memory tool enforces 4000 chars for global scope instead of the default 2200

### Requirement: System prompt guides memory usage

The system prompt SHALL include instructions telling the agent when to save, replace, and skip memories. The guidance SHALL cover: user corrections, environment facts, project conventions, completed work, and explicit requests. It SHALL also list what to skip: trivial info, easily re-discovered facts, raw data, session-specific ephemera.

#### Scenario: Guidance present in prompt
- **WHEN** the agent starts a session with the memory tool available
- **THEN** the system prompt contains a section explaining memory tool usage patterns

