# Automerge Session Storage — Tasks

## Phase 1: Schema + Document Layer

- [x] Add `automerge = "0.7"` dependency to `clankers-session/Cargo.toml`
- [x] Create `crates/clankers-session/src/automerge_store.rs` module
- [x] Define `DocSchema` constants: key names for header, messages, annotations maps
- [x] Implement `create_document(header: &HeaderEntry) -> AutoCommit` — initializes empty doc with header map
- [x] Implement `put_message(doc: &mut AutoCommit, entry: &MessageEntry)` — inserts into messages map
- [x] Implement `put_annotation(doc: &mut AutoCommit, annotation: &AnnotationEntry)` — appends to annotations list
- [x] Define `AnnotationEntry` enum: `Label`, `Compaction`, `ModelChange`, `Branch`, `Resume`, `Custom` — flattened from `SessionEntry` non-message variants
- [x] Implement `read_header(doc: &AutoCommit) -> HeaderEntry` — reads header map
- [x] Implement `read_messages(doc: &AutoCommit) -> Vec<MessageEntry>` — reads all messages from map
- [x] Implement `read_annotations(doc: &AutoCommit) -> Vec<AnnotationEntry>` — reads annotations list
- [x] Implement `to_session_entries(doc: &AutoCommit) -> Vec<SessionEntry>` — reconstructs full entry list for `SessionTree::build()`
- [x] Implement `save_document(doc: &AutoCommit, path: &Path)` — `doc.save()` to `.automerge` file
- [x] Implement `load_document(path: &Path) -> AutoCommit` — `AutoCommit::load()` from file
- [x] Implement `save_incremental(doc: &mut AutoCommit, path: &Path)` — append-only save for fast writes
- [x] Test: create document, put messages, read back, verify round-trip
- [x] Test: put_message with parent_id chain, read_messages preserves parent pointers
- [x] Test: put_annotation for each variant, read back correctly
- [x] Test: save + load round-trip preserves all data
- [x] Test: save_incremental produces loadable document
- [x] Test: to_session_entries output builds a valid SessionTree

## Phase 2: SessionManager Swap

- [x] Add `doc: AutoCommit` field to `SessionManager`
- [x] Change `file_path` to use `.automerge` extension for new sessions
- [x] Rewrite `SessionManager::create()` — initialize Automerge doc, save to disk
- [x] Rewrite `SessionManager::open()` — detect file extension, load `.automerge` via `load_document()`, fall back to JSONL for `.jsonl` files (auto-migrates)
- [x] Rewrite `append_message()` — `put_message()` + `save_incremental()` instead of JSONL append
- [x] Rewrite `load_tree()` — `to_session_entries()` from Automerge doc, feed to `SessionTree::build()`
- [x] Rewrite `record_branch()` — `put_annotation(Branch { ... })` instead of JSONL append
- [x] Rewrite `record_label()` — `put_annotation(Label { ... })` instead of JSONL append
- [x] Add `record_resume()` — `put_annotation(Resume { ... })` for session resume events
- [x] Rewrite `find_branches()` — load entries from doc, existing branch logic unchanged
- [x] Update `resolve_target()` — label lookup reads from annotations instead of raw entries
- [x] Replace `merge_branch()` — remove message cloning, record a merge annotation only
- [x] Replace `merge_selective()` — simplified to plain `append_message` calls with new parent pointers
- [x] Simplify `cherry_pick()` — plain `append_message` calls, iterative DFS instead of recursive `collect_subtree`
- [x] Remove `collect_subtree()` recursive helper — replaced with iterative stack-based DFS
- [x] Update `build_context()` — unchanged API, tree source is now Automerge doc
- [x] `message_count()` — uses persisted_ids (unchanged, already correct)
- [x] Implement `save_compact()` for periodic full save via `doc.save()`
- [x] `SessionManager::open()` on JSONL files auto-migrates to `.automerge`
- [x] Update external callers: `interactive.rs` and `session_setup.rs` use `record_resume()` instead of `store::append_entry`
- [x] Update `store::list_sessions()` / `list_all_sessions()` — include `.automerge` files
- [x] Update `store::find_session_by_id()` — prefers `.automerge` over `.jsonl`
- [x] Update `store::read_session_summary()` — handles `.automerge` files
- [x] Add `store::session_file_path_automerge()` function
- [x] Test: create session, append messages, close, reopen, verify (test_create_and_open_session, test_open_tracks_existing_persisted_ids)
- [x] Test: append_message skips duplicates (test_duplicate_append_is_idempotent)
- [x] Test: record_branch + new messages after branch point (test_record_branch)
- [x] Test: record_label + resolve_target by label name (test_record_label, test_resolve_target_label)
- [x] Test: find_branches returns correct branch metadata (test_find_branches_linear, test_find_branches_with_fork)
- [x] Test: rewind by offset (test_rewind)
- [x] Test: set_active_head to specific message (test_set_active_head)
- [x] Test: build_context returns correct branch messages (test_append_and_build_context)
- [x] Test: cherry_pick single message into another branch (test_cherry_pick_single)
- [x] Test: cherry_pick with children (test_cherry_pick_with_children)
- [x] Test: merge annotation recorded correctly (test_merge_records_metadata)
- [x] Test: save_compact round-trip (test_save_compact)
- [x] Test: read_session_summary from automerge file (test_read_session_summary_automerge)

## Phase 3: Migration + Cleanup

- [x] Implement `migrate_jsonl_to_automerge(jsonl_path: &Path) -> Result<MigrateResult>` in `automerge_store.rs`
- [x] Wire `clankers session migrate <id>` CLI command
- [x] Wire `clankers session migrate --all` CLI command
- [x] Rename original JSONL to `.jsonl.bak` after successful migration
- [x] Skip migration if `.automerge` file already exists
- [x] Print migration summary: count of migrated / skipped / failed
- [x] Update `export.rs` — `load_entries()` handles both `.automerge` and `.jsonl` formats
- [x] Add `export_jsonl()` for JSONL export from any format
- [x] Update `list_sessions()` in `store.rs` — include `.automerge` files (done in Phase 2)
- [x] Update `list_all_sessions()` — include `.automerge` files (done in Phase 2)
- [x] Update `read_session_summary()` — handle `.automerge` files (done in Phase 2)
- [x] Update `find_session_by_id()` — search both extensions, prefer `.automerge` (done in Phase 2)
- [x] Update `purge_sessions()` / `purge_all_sessions()` — delete both extensions + `.jsonl.bak` backups
- [x] Update `import_session()` — accept both `.automerge` and `.jsonl` formats
- [x] Update all tests in `tests/merge.rs` — rewritten in Phase 2
- [x] Verify: 101 tests pass (tree, navigation, context, labels, store, merge, migration)
- [x] Test: migrate JSONL with branches produces identical tree structure
- [x] Test: migrate on already-migrated session skips gracefully
- [ ] _(deferred)_ Delete `merge.rs` — kept for now, contains simplified merge logic via automerge
- [ ] _(deferred)_ Remove `BranchEntry` from `SessionEntry` — still used by tree module for branch resolution
- [ ] _(deferred)_ Move JSONL read functions to `jsonl.rs` — still used by migration + export + legacy open
