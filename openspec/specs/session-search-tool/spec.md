# session-search-tool Specification

## Purpose
TBD - created by archiving change agent-learning-loop. Update Purpose after archive.
## Requirements
### Requirement: Agent can search past sessions

The agent SHALL be able to search past session content by calling the `session_search` tool. The search covers session metadata (first prompt, model, session ID) and, when available, full message content from JSONL files.

#### Scenario: Search by topic
- **WHEN** the agent calls `session_search` with `{"query": "database migration"}`
- **THEN** the tool returns matching sessions sorted by relevance, each with session ID, date, model, first prompt, and a content preview showing the matching context

#### Scenario: No results
- **WHEN** the agent calls `session_search` with a query that matches nothing
- **THEN** the tool returns an empty list

#### Scenario: Limit results
- **WHEN** the agent calls `session_search` with `{"query": "refactor", "limit": 5}`
- **THEN** the tool returns at most 5 matching sessions

### Requirement: Search covers session index metadata

The tool SHALL search the `session_index` table in redb, matching against `first_prompt`, `session_id`, `model`, and `cwd` fields (case-insensitive substring matching).

#### Scenario: Match on model name
- **WHEN** the agent calls `session_search` with `{"query": "opus"}`
- **THEN** sessions that used a model containing "opus" are returned

#### Scenario: Match on working directory
- **WHEN** the agent calls `session_search` with `{"query": "/home/user/myproject"}`
- **THEN** sessions run in that directory are returned

### Requirement: Search can scan JSONL content

When index-level search returns fewer results than requested, the tool SHALL fall back to scanning JSONL session files for the query string. This is slower but catches matches in assistant responses, tool results, and user messages beyond the first prompt.

#### Scenario: Deep search fallback
- **WHEN** the agent calls `session_search` with `{"query": "serde_json::from_slice"}` and the index yields no matches
- **THEN** the tool scans recent JSONL files (up to a configurable limit) for the query and returns matching sessions

#### Scenario: JSONL scan is bounded
- **WHEN** JSONL scanning runs
- **THEN** it scans at most the 100 most recent session files (configurable via `settings.yaml`) to avoid unbounded I/O

### Requirement: Results include content previews

Each search result SHALL include a content preview: the lines surrounding the match, truncated to a reasonable length (default: 200 chars per match, 3 matches per session).

#### Scenario: Preview shows context
- **WHEN** a search matches text in a JSONL file
- **THEN** the result includes the matching line plus one line of context above and below, truncated to 200 chars

### Requirement: Search respects working directory scope

The tool SHALL accept an optional `cwd` parameter. When provided, results are filtered to sessions that ran in that directory. When omitted, all sessions are searched.

#### Scenario: Scoped to current project
- **WHEN** the agent calls `session_search` with `{"query": "auth", "cwd": "/home/user/myproject"}`
- **THEN** only sessions from that working directory are returned

#### Scenario: Global search
- **WHEN** the agent calls `session_search` without a `cwd` parameter
- **THEN** sessions from all working directories are searched

