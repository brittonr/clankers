## Why

Clankers stores full conversation history in Automerge session documents and redb, but the agent has no way to search past sessions for relevant context. When a user references earlier work or the agent encounters a problem it solved before, that knowledge is locked in opaque session files. Hermes solves this with FTS5-indexed session content and LLM-summarized recall — giving the agent cross-session memory beyond its context window.

## What Changes

- Add a full-text search index over session message content in `clankers-db`
- Expose a `session_search` agent tool that queries the index and returns LLM-summarized results
- Index new messages at session write time; backfill existing sessions on first use
- Summaries use a cheap/fast model to avoid burning primary context on raw transcript dumps

## Capabilities

### New Capabilities
- `session-search`: Full-text search over past session transcripts with ranked results and LLM summarization. Agent can recall what was discussed, decided, or built in prior sessions.

### Modified Capabilities

## Impact

- `crates/clankers-db/` — new `session_search` module with FTS index tables, indexing on write, and backfill migration
- `crates/clankers-agent/` — new `session_search` tool registration and dispatch
- `crates/clankers-session/` — hook into session write path to index new messages
- `crates/clankers-provider/` — auxiliary model call for summarization (reuse existing provider infra)
- Disk: FTS index adds storage proportional to session history size
