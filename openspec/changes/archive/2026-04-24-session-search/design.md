## Context

Clankers stores sessions as Automerge documents with full message history. The `clankers-db` crate (redb) stores a session index, audit log, usage data, and cross-session memory — but no searchable text index of conversation content. The agent currently has no tool to recall what happened in past sessions.

Hermes uses SQLite FTS5 for full-text search over session messages, then sends top-matching session transcripts to a cheap model for summarization. This keeps the primary model's context clean while providing focused recall.

## Goals / Non-Goals

**Goals:**
- Full-text search over all past session message content
- Ranked results grouped by session with metadata (date, model, duration)
- LLM-summarized recall using a cheap auxiliary model, not raw transcript dumps
- Incremental indexing: new messages indexed at write time
- Backfill migration for existing session files on first use
- Agent tool `session_search` with query string input

**Non-Goals:**
- Semantic/embedding-based search (FTS is sufficient and doesn't require a local model)
- Real-time streaming index updates (batch at session save is fine)
- Searching across multiple machines (local DB only)
- Replacing the existing memory system — session search supplements it

## Decisions

**redb for FTS rather than SQLite:** clankers already uses redb for all structured storage. Adding SQLite solely for FTS5 would create a second database to manage. Instead, implement prefix-based text search over redb tables using an inverted index approach: tokenize messages, store term→(session_id, message_id, position) mappings. This is simpler than FTS5 but sufficient for keyword search with ranking.

**Alternative — tantivy:** If prefix search on redb proves too limited, tantivy (Rust full-text search library) gives proper BM25 ranking, phrase queries, and fuzzy matching without pulling in SQLite. It stores its own index files alongside the redb database. This is the recommended approach if keyword matching alone is insufficient.

**Summarization via auxiliary model:** Reuse the existing provider infrastructure to make a separate completion call with a cheap model (configurable, defaults to haiku). The summarization prompt asks for a focused summary of what was discussed/decided/built in the matched session, not a generic summary.

**Token budget for summarization input:** Truncate session transcripts to ~50k chars centered around match positions (similar to Hermes' `_truncate_around_matches`). Summarization output capped at ~2k tokens per session, max 3 sessions per search.

**Index location:** `~/.clankers/agent/search_index/` alongside `clankers.db`.

## Risks / Trade-offs

- **Index size:** Full-text index adds 20-40% of session text size in storage. For heavy users with thousands of sessions this could be hundreds of MB. Acceptable given disk is cheap.
- **Backfill latency:** First-use migration scans all Automerge session files to populate the index. Could take seconds to minutes depending on history size. Should run in background with progress reporting.
- **Summarization cost:** Each search costs one cheap-model API call per matched session. With max 3 sessions this is ~$0.001 per search. Acceptable.
- **tantivy dependency:** Adds a non-trivial Rust dependency. Build time impact should be measured. The crate is well-maintained and commonly used.
