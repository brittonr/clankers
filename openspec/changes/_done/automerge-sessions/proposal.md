# automerge-sessions

## Intent

Session persistence is JSONL append-only files. Branching, merging, and
cherry-pick are implemented as manual tree operations — 246 lines of
`merge.rs` that clone messages with new IDs and append them as separate
entries. Two clients writing to the same session file will corrupt it.
The branch/merge code is intrinsically sequential: it reads the whole
file, computes unique messages, writes clones back. No concurrent access
story at all.

The session tree already IS a CRDT conceptually — messages are immutable
nodes with parent pointers forming a DAG, branches are implicit in the
topology, merging means making both branches visible. Automerge stores
exactly this kind of structure natively. Switching the storage layer
eliminates the manual merge code and makes concurrent writes from
multiple clients correct by construction.

This also unblocks multi-client daemon sessions. Right now `attach`
can read events but can't safely write to the session from two TUI
clients simultaneously. With Automerge, concurrent appends from daemon
+ client just merge.

## Scope

### In scope

- `automerge` crate dependency in `clankers-session`
- Automerge document schema: header map, messages map (keyed by
  MessageId), annotations list (labels, compactions, model changes)
- `SessionManager` backed by `automerge::AutoCommit` instead of JSONL I/O
- `SessionTree` built from Automerge document state
- Merge and cherry-pick simplified to plain Automerge writes
- Migration tool: `clankers session migrate <id>` converts JSONL to
  `.automerge`
- JSONL preserved as export-only format
- Document compaction (`doc.save()`) for file size management
- All existing session tests passing against the new backend

### Out of scope

- iroh-docs sync (separate spec, builds on this)
- Todo list / napkin as CRDTs (separate spec)
- Aspen backend integration (separate spec)
- Multi-process concurrent write (this spec makes it structurally
  possible; the daemon wiring is separate work)
- Changes to the controller, TUI, or protocol layers

## Approach

Three phases:

1. **Schema + document layer** — define the Automerge document structure,
   implement read/write primitives, conversion between Automerge maps
   and the existing `SessionEntry`/`MessageEntry` types.

2. **SessionManager swap** — replace JSONL file I/O in `SessionManager`
   with Automerge document operations. `append_message` becomes an
   Automerge map put. `load_tree` reads from the document. `merge_branch`
   is replaced by Automerge's native merge (both branches already visible
   in the DAG). `cherry_pick` is still a write operation but just creates
   new messages — no special merge logic needed.

3. **Migration + cleanup** — `clankers session migrate` converts JSONL
   files. `clankers session export` outputs JSONL for interop. Remove
   the 246-line `merge.rs` manual merge code. Document compaction via
   `doc.save()` to squash change history.
