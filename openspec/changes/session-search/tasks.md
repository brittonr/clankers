## 1. Search index infrastructure

- [ ] 1.1 Add `tantivy` to workspace dependencies in `Cargo.toml`
- [ ] 1.2 Create `crates/clankers-db/src/search_index.rs` module with schema definition (session_id, message_id, timestamp, role, content fields)
- [ ] 1.3 Implement `SearchIndex::open(path)` that creates or opens a tantivy index at `~/.clankers/agent/search_index/`
- [ ] 1.4 Implement `SearchIndex::index_message(session_id, message_id, role, content, timestamp)` for single-message indexing
- [ ] 1.5 Implement `SearchIndex::search(query, limit) -> Vec<SearchHit>` with BM25 ranking returning session_id, message_id, score, snippet
- [ ] 1.6 Wire `SearchIndex` into `Db` struct so it's accessible alongside redb tables

## 2. Incremental indexing

- [ ] 2.1 Hook into the session save path in `crates/clankers-session/` to call `SearchIndex::index_message` for new messages
- [ ] 2.2 Deduplicate: skip messages already in the index (check by message_id)

## 3. Backfill migration

- [ ] 3.1 Implement `SearchIndex::backfill(session_store)` that scans all Automerge session files and indexes their messages
- [ ] 3.2 Track backfill progress (last processed session) so it can resume if interrupted
- [ ] 3.3 Run backfill in a background tokio task on first search if index is empty

## 4. Summarization

- [ ] 4.1 Add `session_search_summary_model` config option in `crates/clankers-config/src/settings.rs` (default: haiku)
- [ ] 4.2 Implement `summarize_session(transcript, query, provider) -> String` that sends a truncated transcript to the auxiliary model with a focused summarization prompt
- [ ] 4.3 Implement transcript truncation: center a 50k-char window around match positions
- [ ] 4.4 Fallback: if summarization fails, return truncated raw excerpts (500 chars around each match)

## 5. Agent tool

- [ ] 5.1 Define `session_search` tool schema in `crates/clankers-agent/src/tool/` with `query: String` parameter
- [ ] 5.2 Implement tool dispatch: query index → group by session → take top 3 → summarize each → format response
- [ ] 5.3 Register tool in `crates/clankers-agent/src/tool/mod.rs`

## 6. Tests

- [ ] 6.1 Unit test: index a few messages, search, verify ranking
- [ ] 6.2 Unit test: backfill from mock session files
- [ ] 6.3 Integration test: end-to-end tool call with mock provider for summarization
