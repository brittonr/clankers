# Automerge Session Storage — Plan

## Problem

Session persistence is a JSONL append log with manual tree operations on
top. The `merge.rs` code (246 lines) clones messages with new IDs and
appends them — a hand-rolled CRDT that doesn't know it's one. Two writers
hitting the same `.jsonl` file produce garbled interleaved lines. Branch
and merge operations read the entire file, compute diffs, write clones.
No concurrent access, no conflict resolution.

The session tree is already a DAG of immutable nodes with parent pointers.
That's the exact data model Automerge stores. The "merge" operation in
Automerge is just: both branches are already visible in the document.
No cloning, no new IDs, no manual diff.

## Current State

### clankers-session crate (1,641 lines + 579 tree + 1,082 tests)

```
lib.rs       386  SessionManager — create/open/append/load_tree/build_context
entry.rs     237  SessionEntry enum (Header, Message, Compaction, Branch, etc.)
store.rs     496  JSONL file I/O — read_entries, append_entry, list/purge/import
merge.rs     246  merge_branch, merge_selective, cherry_pick (manual cloning)
context.rs    20  build_messages_for_branch (walks tree, extracts AgentMessages)
export.rs    233  export to markdown/text/JSON
error.rs      23  SessionError type

tree/
  mod.rs      357  SessionTree — build from entries, O(1) lookup via index
  navigation.rs 152  walk_branch, find_latest_leaf, find_all_leaves, get_children
  query.rs     70  is_branch_point, find_divergence_point, find_unique_messages
```

### Call sites

- `SessionController` (clankers-controller): owns `Option<SessionManager>`,
  calls `append_message` via `persistence.rs`
- `EventLoopRunner` (src/modes/): passes `&mut Option<SessionManager>` to
  event handlers and slash commands
- Slash commands (`src/slash_commands/handlers/branching.rs`): calls
  `record_branch`, `merge_branch`, `merge_selective`, `cherry_pick`,
  `load_tree`, `set_active_head`, `rewind`, `find_branches`
- `session_setup.rs`: calls `SessionManager::create` and `::open`
- `session_store.rs` (daemon): session listing and metadata

### What stays the same

- `SessionTree` — still built the same way, still queried the same way.
  Only the source changes (Automerge doc instead of `Vec<SessionEntry>`).
- `SessionManager` public API — same methods, same signatures. Callers
  don't know the backend changed.
- `entry.rs` types — `SessionEntry`, `MessageEntry`, etc. still exist for
  in-memory representation and JSONL export.
- `context.rs` — unchanged, operates on `SessionTree`.
- `export.rs` — unchanged, reads from `SessionTree`.
- `tree/` — navigation and query modules unchanged.

### What changes

- `store.rs` — JSONL I/O functions become the JSONL export path only.
  New `automerge_store.rs` handles document persistence.
- `merge.rs` — 246 lines deleted. `merge_branch` becomes "both branches
  are already visible." `cherry_pick` becomes a plain `append_message`
  with a new parent pointer.
- `SessionManager` internals — `file_path` becomes doc path (`.automerge`
  extension). `persisted_ids` populated from Automerge doc on open. File
  I/O replaced with `doc.save()` / `automerge::AutoCommit::load()`.
- `lib.rs` — `SessionManager::create` initializes an Automerge doc
  instead of writing a JSONL header line. `SessionManager::open` loads
  an Automerge doc instead of parsing JSONL.

## Automerge Document Schema

```
{
  "header": {
    "session_id": "abc123",
    "created_at": "2026-03-13T15:00:00Z",
    "cwd": "/home/user/project",
    "model": "claude-sonnet-4-20250514",
    "version": "0.1.0",
    "agent": null,
    "parent_session_id": null,
    "worktree_path": null,
    "worktree_branch": null
  },
  "messages": {
    "<message-id>": {
      "parent_id": "<message-id> | null",
      "message_json": "<serde_json serialized AgentMessage>",
      "timestamp": "<iso8601>"
    }
  },
  "annotations": [
    {
      "kind": "label",
      "target_message_id": "<message-id>",
      "label": "important",
      "timestamp": "<iso8601>"
    },
    {
      "kind": "compaction",
      "compacted_range": ["<id>", ...],
      "summary": "...",
      "tokens_before": 1000,
      "tokens_after": 100,
      "timestamp": "<iso8601>"
    },
    {
      "kind": "model_change",
      "from_model": "haiku",
      "to_model": "sonnet",
      "reason": "user_request",
      "timestamp": "<iso8601>"
    }
  ]
}
```

Messages are stored in a map keyed by MessageId. The parent pointer is a
string field within each message value. This gives O(1) lookup by ID
(Automerge maps are hash-indexed) and makes concurrent inserts from
different writers conflict-free — each writer puts a different key.

Annotations are a list because they're ordered and append-only. Labels,
compactions, model changes, branch markers, and custom entries all go
here. They reference messages by ID.

`message_json` is the `AgentMessage` serialized as a JSON string. We
don't decompose it into Automerge sub-maps because we never merge
partial message edits — messages are immutable once written. Storing
as a JSON string avoids mapping every `Content` variant into Automerge
types.

## Merge Simplification

### Before (JSONL merge.rs)

1. `merge_branch(source_leaf, target_leaf)`:
   - Read entire file → build tree → `find_unique_messages`
   - For each unique message: generate new ID, clone `AgentMessage` with
     new ID, append `MessageEntry` with parent chain pointing to target
   - Append a `Custom("merge")` entry as metadata
   - 60 lines of code per merge variant, 246 total for three methods

2. `cherry_pick(message_id, target_leaf, with_children)`:
   - Collect subtree via recursive DFS
   - Re-parent with ID remapping via `HashMap<old_id, new_id>`
   - Same clone-and-append pattern

### After (Automerge)

1. `merge_branch` — deleted. Both branches are already visible in the
   `messages` map. The tree structure is determined by parent pointers.
   "Merging" just means the user switches their active leaf to see both
   branches. If you want a merge commit (a message with two parents or a
   parent from the other branch), that's just `append_message` with the
   desired parent pointer.

2. `cherry_pick(message_id, target_leaf)` — becomes `append_message` with
   the cherry-picked message's content and `parent_id` set to
   `target_leaf`. One call, no subtree walking, no ID remapping. If
   `with_children` is true, walk the subtree and append each message
   individually — but still just plain `append_message` calls.

The `BranchEntry` type is removed. Branches are implicit: any message
with multiple children is a branch point. The tree module already detects
this via `is_branch_point()`.

## File Format

- New sessions: `<timestamp>_<session_id>.automerge`
- `doc.save()` produces a single binary blob — all changes compacted
- `doc.save_incremental()` for fast appends without full rewrite
- On open: `AutoCommit::load(&bytes)` reconstitutes the document

Strategy: save incrementally on each `append_message` (fast, append-only).
Periodically do a full `doc.save()` to compact the change log. The
compaction can happen on session close or when incremental size exceeds
a threshold.

## Migration

`clankers session migrate <id>`:
1. Find the JSONL file for the given session ID
2. `read_entries()` to get `Vec<SessionEntry>`
3. Create a new `AutoCommit`, populate header + messages + annotations
4. `doc.save()` to `<same-path-stem>.automerge`
5. Rename original to `<original>.jsonl.bak`

`clankers session migrate --all`:
1. Walk all JSONL files in sessions directory
2. Migrate each, skip if `.automerge` already exists
3. Print summary (migrated / skipped / failed)

## Open Questions

**Q: Keep JSONL as a parallel write path for a transition period?**

No. Cut over cleanly. The JSONL code stays for `export` and `migrate`
(reading old files), but new sessions write only Automerge. A transition
period where both formats are written doubles the I/O and doubles the
bug surface.

**Q: Store `AgentMessage` as Automerge sub-documents or JSON strings?**

JSON strings. Messages are write-once, never partially updated. Decomposing
`Content::ToolUse { name, id, input }` into Automerge maps buys nothing
since we never merge two versions of the same tool call. The JSON string
round-trips through `serde_json` which is already proven across the
codebase.

**Q: How does compaction interact with Automerge?**

Context compaction (replacing old tool results with summaries) is a
session-level annotation, not a document edit. The `CompactionEntry`
becomes an annotation that lists which message IDs were compacted and
provides the summary. The original messages stay in the document — the
compaction annotation tells `build_context()` to substitute the summary.
This is a semantic change from the JSONL approach where compaction wrote
a new entry, but the effect is identical.

Automerge document compaction (`doc.save()` squashing the internal change
log) is orthogonal. It reduces file size by merging incremental changes
into a single snapshot. No semantic impact on session data.
