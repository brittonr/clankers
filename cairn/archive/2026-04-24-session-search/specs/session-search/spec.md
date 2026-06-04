## ADDED Requirements

### Requirement: Full-text search over session history
The system SHALL maintain a full-text search index over all past session message content. The index SHALL support keyword queries and return results ranked by relevance.

#### Scenario: Search finds matching sessions
- **WHEN** the agent calls `session_search` with query "database migration"
- **THEN** the system returns sessions containing those terms, ranked by relevance, with session metadata (date, model, message count)

#### Scenario: No results found
- **WHEN** the agent calls `session_search` with a query matching no sessions
- **THEN** the system returns an empty result set with a message indicating no matches

---

### Requirement: Incremental index updates
The system SHALL index new messages when sessions are saved. The index SHALL NOT require a full rebuild when new sessions are added.

#### Scenario: New session is indexed on save
- **WHEN** a session is saved with new messages
- **THEN** the new message content is added to the search index and is immediately searchable

---

### Requirement: Backfill migration for existing sessions
The system SHALL backfill the search index from existing session files on first use. The migration SHALL run in the background without blocking the agent.

#### Scenario: First-time index build
- **WHEN** the agent calls `session_search` and no index exists
- **THEN** the system begins backfilling from existing session files and reports progress
- **THEN** partial results are available as backfill progresses

---

### Requirement: LLM-summarized recall
The system SHALL summarize matched session transcripts using an auxiliary cheap model rather than injecting raw transcripts into the primary context. Each matched session SHALL produce a focused summary of what was discussed, decided, or built.

#### Scenario: Search returns summarized results
- **WHEN** the agent calls `session_search` and matches are found
- **THEN** each matched session's transcript is sent to an auxiliary model for summarization
- **THEN** the tool returns per-session summaries (not raw transcripts)

#### Scenario: Auxiliary model unavailable
- **WHEN** no auxiliary model is configured or the summarization call fails
- **THEN** the system falls back to returning truncated raw excerpts centered around match positions

---

### Requirement: Result limits
The system SHALL return at most 3 session summaries per search query. Each session transcript sent for summarization SHALL be truncated to at most 50,000 characters centered around match positions.

#### Scenario: Many sessions match
- **WHEN** a query matches more than 3 sessions
- **THEN** only the top 3 by relevance score are summarized and returned
