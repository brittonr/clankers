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

- [ ] Add `doc: Option<AutoCommit>` field to `SessionManager`
- [ ] Change `file_path` to use `.automerge` extension for new sessions
- [ ] Rewrite `SessionManager::create()` — initialize Automerge doc, save to disk
- [ ] Rewrite `SessionManager::open()` — detect file extension, load `.automerge` via `load_document()`, fall back to JSONL for `.jsonl` files
- [ ] Rewrite `append_message()` — `put_message()` + `save_incremental()` instead of JSONL append
- [ ] Rewrite `load_tree()` — `to_session_entries()` from Automerge doc, feed to `SessionTree::build()`
- [ ] Rewrite `record_branch()` — `put_annotation(Branch { ... })` instead of JSONL append
- [ ] Rewrite `record_label()` — `put_annotation(Label { ... })` instead of JSONL append
- [ ] Rewrite `find_branches()` — load tree from doc, existing branch logic unchanged
- [ ] Update `resolve_target()` — label lookup reads from annotations instead of raw entries
- [ ] Replace `merge_branch()` — remove message cloning, record a merge annotation only
- [ ] Replace `merge_selective()` — remove message cloning, append selected messages with new parent pointers
- [ ] Simplify `cherry_pick()` — plain `append_message` calls, no ID remapping HashMap
- [ ] Remove `collect_subtree()` recursive helper — use `SessionTree::walk_branch` or iterate children directly
- [ ] Update `build_context()` — unchanged API, tree source is now Automerge doc
- [ ] Update `message_count()` — read from doc messages map length
- [ ] Implement periodic full save: `doc.save()` on session close or when incremental size > 1MB
- [ ] Verify: `SessionManager::open()` on JSONL files still works (read-only backward compat)
- [ ] Test: create session, append 10 messages, close, reopen, verify all messages present
- [ ] Test: append_message skips duplicates (persisted_ids check)
- [ ] Test: record_branch + new messages after branch point
- [ ] Test: record_label + resolve_target by label name
- [ ] Test: find_branches returns correct branch metadata
- [ ] Test: rewind by offset
- [ ] Test: set_active_head to specific message
- [ ] Test: build_context returns correct branch messages
- [ ] Test: cherry_pick single message into another branch
- [ ] Test: cherry_pick with children
- [ ] Test: merge annotation recorded correctly
- [ ] Test: open JSONL file via backward-compat path

## Phase 3: Migration + Cleanup

- [ ] Implement `migrate_jsonl_to_automerge(jsonl_path: &Path) -> Result<PathBuf>` in `automerge_store.rs`
- [ ] Wire `clankers session migrate <id>` CLI command
- [ ] Wire `clankers session migrate --all` CLI command
- [ ] Rename original JSONL to `.jsonl.bak` after successful migration
- [ ] Skip migration if `.automerge` file already exists
- [ ] Print migration summary: count of migrated / skipped / failed
- [ ] Update `clankers session export` — read from Automerge doc, output JSONL
- [ ] Update `list_sessions()` in `store.rs` — include `.automerge` files
- [ ] Update `list_all_sessions()` — include `.automerge` files
- [ ] Update `read_session_summary()` — handle `.automerge` files
- [ ] Update `find_session_by_id()` — search both extensions
- [ ] Update `purge_sessions()` / `purge_all_sessions()` — delete both extensions
- [ ] Update `import_session()` — accept both formats
- [ ] Delete `merge.rs` (246 lines) — merge logic replaced by annotations + direct writes
- [ ] Remove `BranchEntry` from `SessionEntry` — branches are implicit in the DAG, explicit via annotations
- [ ] Move remaining JSONL read functions to a `jsonl.rs` module (used only by migrate + export + legacy open)
- [ ] Update all tests in `tests/merge.rs` — rewrite against new merge semantics
- [ ] Verify: all existing tests in `tests/` pass (tree, navigation, context, labels, store)
- [ ] Verify: `clankers session migrate` on a real session with branches produces identical tree
- [ ] Verify: export from migrated session matches original JSONL content
- [ ] Test: migrate --all on directory with mixed JSONL/automerge files
- [ ] Test: migrate on already-migrated session skips gracefully
