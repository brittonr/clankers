# CRDT Session Layer

## Purpose

Replace JSONL-based session persistence with Automerge CRDT documents.
The session tree is already an append-only DAG of immutable entries with
unique IDs and parent pointers — that's what Automerge does natively.
Switching eliminates manual merge/cherry-pick/selective-merge code and
enables concurrent writes from multiple agents or clients without
conflict.

## Requirements

### Session document structure

The system MUST represent each session as a single Automerge document
with the following structure:

```rust
// Automerge document schema (conceptual — stored as Automerge ops)
struct SessionDocument {
    /// Session metadata
    header: {
        session_id: String,
        model: String,
        created_at: String,       // ISO 8601
        cwd: String,
        worktree: Option<String>,
    },

    /// Messages keyed by MessageId — Automerge map
    /// Concurrent inserts with different keys merge automatically
    messages: Map<MessageId, MessageEntry>,

    /// Root message IDs (messages with no parent) — Automerge list
    roots: List<MessageId>,

    /// Labels, compactions, model changes — keyed by ID
    annotations: Map<String, AnnotationEntry>,
}
```

GIVEN two agents concurrently append messages to the same session document
WHEN agent A writes `MessageEntry { id: "a1", parent_id: "root" }`
AND agent B writes `MessageEntry { id: "b1", parent_id: "root" }`
THEN both entries appear in the merged document as children of "root"
AND no manual merge step is needed

### MessageEntry in Automerge

Each message MUST be stored as an Automerge map entry keyed by its
`MessageId`. The entry contains the same fields as today's `MessageEntry`.

```rust
struct MessageEntry {
    id: MessageId,
    parent_id: Option<MessageId>,
    role: String,          // "user", "assistant", "tool_call", "tool_result"
    content: String,       // serialized message content
    timestamp: String,     // ISO 8601
    model: Option<String>, // model that generated this (for assistant messages)
}
```

GIVEN a message is appended to the session
WHEN it is written to the Automerge document
THEN it creates a new key in the `messages` map
AND the key is the message's `MessageId`
AND concurrent writes to different keys never conflict

### Branching is free

Branching MUST require no special operation. A branch is simply a
message whose `parent_id` points to an existing message that already
has other children.

GIVEN a conversation at message M5 (linear chain: M1→M2→M3→M4→M5)
WHEN the user branches at M3 with a new prompt
THEN a new message M6 is written with `parent_id: M3`
AND the session tree now has two branches: M3→M4→M5 and M3→M6
AND no `BranchEntry` or `SessionEntry::Branch` is needed

This eliminates `BranchEntry` from `SessionEntry`. The branch structure
is implicit in the parent-pointer DAG, exactly as it is today in
`SessionTree` — but now the storage format matches the logical model.

### Merging is Automerge merge

The `merge_branch()`, `merge_selective()`, and `cherry_pick()` methods
on `SessionManager` MUST be replaced by Automerge's native merge.

GIVEN branch A (messages A1→A2→A3) and branch B (messages B1→B2)
WHEN a user merges branch A into branch B
THEN the merge appends a `MergeMarker` annotation pointing to both tips
AND the TUI displays both branches' messages in the merged view
AND no message copying or ID remapping is needed

For cherry-pick (copying specific messages from one branch to another),
the system MUST still create new messages with new IDs and re-parented
pointers — but these are regular Automerge writes, not a special code
path.

GIVEN branch A has messages [A1, A2, A3]
WHEN the user cherry-picks A2 onto branch B's tip
THEN a new message A2' is created with `parent_id: B_tip`
AND A2' has the same content as A2 but a new MessageId

### SessionManager wraps Automerge

The `SessionManager` MUST wrap an `automerge::AutoCommit` document
instead of a file path.

```rust
struct SessionManager {
    doc: automerge::AutoCommit,
    /// Path to save/load the document
    file_path: PathBuf,
    /// Cached tree (rebuilt on load, updated incrementally on writes)
    cached_tree: Option<SessionTree>,
}
```

The public interface MUST remain compatible:

```rust
impl SessionManager {
    fn create(dir, label, model, tag, cwd, worktree) -> Result<Self>;
    fn load(path) -> Result<Self>;
    fn append_message(id, parent_id, message) -> Result<()>;
    fn load_tree() -> Result<SessionTree>;
    fn save() -> Result<()>;  // persist Automerge doc to disk
}
```

GIVEN code that calls `session_mgr.append_message(...)`
WHEN the session manager is backed by Automerge
THEN the call succeeds and the message appears in the tree
AND existing callers do not need to change

### Concurrent writes

The Automerge document MUST support concurrent writes from:
1. The `SessionController` (agent appending messages during a turn)
2. A TUI client (adding labels, bookmarks, or annotations)
3. Another agent (in multi-agent sessions sharing a document)

GIVEN the daemon's SessionController writes a tool result
AND a TUI client simultaneously adds a label to an earlier message
WHEN the Automerge document merges both changes
THEN both the tool result and the label are present
AND neither write blocks or overwrites the other

### iroh-docs sync

The system SHOULD sync session documents between daemon and clients
via iroh-docs for real-time replication.

```
Daemon SessionController
    │
    ▼ (Automerge changes)
iroh-docs namespace (per-session)
    │
    ▼ (range-based set reconciliation)
TUI Client (local replica)
```

GIVEN a TUI client attached to a daemon session
WHEN the agent appends a message on the daemon
THEN the Automerge change propagates via iroh-docs
AND the TUI's local replica updates
AND the TUI renders the new message

GIVEN a TUI client is disconnected (offline)
WHEN it reconnects
THEN iroh-docs syncs the missing changes
AND the TUI's session state catches up without replaying history

This is complementary to the `DaemonEvent` stream — events drive
real-time rendering, iroh-docs sync drives persistence consistency.
The TUI can render from events for low latency and use the Automerge
doc as the source of truth for history.

### Offline-first

The system MUST support offline operation with later reconciliation.

GIVEN a TUI in embedded mode (no daemon) writes to a local session
AND a daemon on another machine also writes to the same session
WHEN the two Automerge documents are merged later
THEN all messages from both sides appear in the merged tree
AND the tree structure is consistent (no orphaned messages)

### File format

Session documents MUST be saved as Automerge binary format (`.automerge`)
instead of JSONL (`.jsonl`).

The system SHOULD provide a migration tool to convert existing JSONL
sessions to Automerge documents.

GIVEN a session directory with `session.jsonl`
WHEN `clankers session migrate <id>` is run
THEN the JSONL entries are loaded and written to a new Automerge document
AND the original JSONL file is preserved as `session.jsonl.bak`

### What stays as JSONL

The system SHOULD keep JSONL as a fallback format for:
- Export (`clankers session export` outputs human-readable JSONL)
- Compatibility with older clankers versions
- Debugging (Automerge binary is not human-readable)

### Compaction

Session compaction MUST work within Automerge.

GIVEN a session with 500 messages
WHEN the agent triggers compaction
THEN a `CompactionEntry` annotation is written to the Automerge doc
AND the compacted messages remain in the document history (Automerge
preserves all changes) but are excluded from the active conversation
context

Automerge's own document compaction (`doc.save()` vs `doc.save_with()`)
can be used separately to reduce file size by squashing the change
history.

## What Automerge does NOT replace

### Agent conversation messages (turn-by-turn)

The sequential agent turn loop (user → LLM → tools → LLM → tools → done)
MUST NOT use Automerge's conflict resolution for message ordering within
a single turn. Within a turn, messages are strictly ordered by the agent.
Automerge is for the tree structure across turns and branches, not for
intra-turn sequencing.

### Settings

Settings MUST NOT be stored in Automerge. They change rarely, are small,
and last-writer-wins is the correct semantics. CRDT merge of settings
could produce nonsensical combinations.

### Ephemeral tool state

In-progress tool execution state (streaming output buffers, progress
counters) MUST NOT be stored in Automerge. These are ephemeral and
flow through the `DaemonEvent` stream.
