# CRDT Shared State

## Purpose

Define which non-session state benefits from Automerge CRDT semantics.
These are smaller documents that multiple agents or clients mutate
concurrently.

## Requirements

### Todo list

The agent todo list MUST be an Automerge document shared between
the SessionController and connected clients.

```rust
// Automerge document schema
struct TodoDocument {
    items: List<TodoItem>,
}

struct TodoItem {
    id: u64,
    text: String,
    status: String,  // "todo", "in_progress", "done", "blocked"
    note: Option<String>,
    created_at: String,
    updated_at: String,
}
```

GIVEN an agent adds a todo item via the todo tool
AND a TUI client marks a different item as "done" simultaneously
WHEN the Automerge document merges both changes
THEN the new item appears and the status change is applied
AND no channel plumbing (oneshot sender/receiver) is needed

This eliminates the `todo_tx` / `todo_rx` channel pair in
`EventLoopRunner`. The agent writes directly to the Automerge doc,
the TUI reads from its local replica. Changes propagate via iroh-docs
or in-process Automerge sync.

GIVEN the TodoTool receives `TodoAction::Add { text: "fix tests" }`
WHEN it writes to the Automerge todo document
THEN the item appears in the TUI's todo panel without a channel round-trip

GIVEN the TUI user marks item 3 as "done"
WHEN it writes to the Automerge todo document
THEN the agent can read the updated status on its next tool call

### Todo list backward compatibility

The system MUST support the existing `TodoAction` / `TodoResponse`
interface as a compatibility layer over the Automerge document.

GIVEN code that calls the todo tool with `TodoAction::SetStatus`
WHEN the todo backend is Automerge
THEN the call translates to an Automerge map update on the item
AND a `TodoResponse::Updated` is returned

### Napkin

The per-repo napkin (`.agent/napkin.md`) SHOULD be an Automerge text
document when multiple agents work on the same repository.

GIVEN two agents working in the same repo
WHEN agent A appends a correction to the napkin
AND agent B appends a different correction concurrently
THEN both corrections appear in the merged napkin
AND neither overwrites the other

The napkin MUST fall back to a regular file when Automerge sync is
not available (embedded mode, no iroh-docs). Automerge text can be
exported to/imported from markdown.

GIVEN a repo with a legacy `.agent/napkin.md` file
WHEN the first Automerge-aware session starts
THEN the file contents are imported into an Automerge text document
AND subsequent writes go to the Automerge doc
AND the markdown file is regenerated on save for non-Automerge readers

### Peer registry

The peer registry (`~/.clankers/agent/peers.json`) SHOULD be an
Automerge document synced across machines via iroh-docs.

```rust
struct PeerRegistryDocument {
    peers: Map<NodeId, PeerInfo>,
}
```

GIVEN machine A adds a peer "build-server" with node ID xyz
WHEN the peer registry syncs to machine B via iroh-docs
THEN machine B can `clankers attach --remote build-server` without
manually copying the peers.json

Merge semantics for peer entries:
- `name`: last-writer-wins (Automerge default for register)
- `capabilities`: union (Automerge list, append-only)
- `last_seen`: max timestamp wins (application-level merge in a hook)

### What does NOT become a CRDT

The following MUST NOT be stored in Automerge:

**Settings** — last-writer-wins is correct. CRDT merge of `model: "sonnet"`
and `model: "opus"` would pick one arbitrarily, confusing the user.

**Auth tokens (redb)** — security-critical, must be authoritative, not
eventually consistent. A revoked token must be immediately revoked
everywhere, not merged back in by a stale replica.

**Streaming output buffers** — ephemeral, per-tool, high-frequency writes.
Automerge overhead is unjustified for data that lives for seconds.

**Session JSONL export format** — JSONL stays as the human-readable export
and compatibility format. The Automerge doc is the live storage; JSONL is
a snapshot.

### Automerge document lifecycle

Each Automerge document MUST follow this lifecycle:

1. **Create** — initialize empty Automerge doc with schema
2. **Write** — apply changes via `AutoCommit` transactions
3. **Save** — serialize to `.automerge` binary file (or aspen KV)
4. **Sync** — exchange changes via iroh-docs (when connected)
5. **Load** — deserialize from file (or aspen KV) on session resume
6. **Merge** — combine two diverged replicas into one

GIVEN a session document saved to `session.automerge`
WHEN the daemon restarts and loads it
THEN the full session tree is reconstructed from the Automerge doc
AND all branches, annotations, and merge history are preserved

### Document storage location

Automerge documents MUST be stored alongside existing session files:

```
~/.local/share/clankers/sessions/<session-id>/
├── session.automerge    # primary Automerge document
├── session.jsonl.bak    # pre-migration backup (if migrated)
├── todo.automerge       # todo list document
└── meta.json            # session metadata (non-CRDT)
```
