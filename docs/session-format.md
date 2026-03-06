# Session JSONL Format

Clankers sessions are stored as append-only JSONL (newline-delimited JSON) files. Each line is a `SessionEntry` with a discriminated `type` field. This format supports conversation branching, compaction, labeling, and extensibility via custom entries.

## Entry Types

### Header

**Required first entry.** Written once at session creation.

```json
{
  "type": "Header",
  "session_id": "abc12345",
  "created_at": "2024-03-06T05:30:00Z",
  "cwd": "/home/user/project",
  "model": "claude-sonnet-4-5",
  "version": "0.1.0",
  "agent": "worker",
  "parent_session_id": "def67890",
  "worktree_path": "/tmp/clankers-worktree-abc12345",
  "worktree_branch": "session/abc12345"
}
```

Fields:
- `session_id` — unique session identifier
- `created_at` — ISO 8601 timestamp
- `cwd` — working directory at session start
- `model` — initial model name
- `version` — clankers version
- `agent` (optional) — agent definition name (e.g., "worker", "reviewer")
- `parent_session_id` (optional) — session this was forked from
- `worktree_path` (optional) — git worktree path
- `worktree_branch` (optional) — git worktree branch name

### Message

**Core conversation message.** Forms a tree via `parent_id` links.

```json
{
  "type": "Message",
  "id": "msg_001",
  "parent_id": null,
  "message": { /* AgentMessage union */ },
  "timestamp": "2024-03-06T05:30:05Z"
}
```

Fields:
- `id` — unique message identifier (8-char hex)
- `parent_id` — parent message ID (null for root messages)
- `message` — one of the `AgentMessage` variants (see below)
- `timestamp` — ISO 8601 timestamp

#### AgentMessage Variants

- **User** — user input
  ```json
  {
    "type": "User",
    "id": "msg_001",
    "content": [{"type": "Text", "text": "Fix the parser"}],
    "timestamp": "2024-03-06T05:30:05Z"
  }
  ```

- **Assistant** — model response
  ```json
  {
    "type": "Assistant",
    "id": "msg_002",
    "content": [{"type": "Text", "text": "I'll analyze the parser..."}],
    "model": "claude-sonnet-4-5",
    "usage": {"input_tokens": 1200, "output_tokens": 450},
    "stop_reason": "stop",
    "timestamp": "2024-03-06T05:30:10Z"
  }
  ```

- **ToolResult** — result from tool execution
- **BashExecution** — bash command output (stored for display)
- **Custom** — extensible custom message with `kind` discriminator
- **BranchSummary** — branch point summary
- **CompactionSummary** — context compaction summary

### Compaction

**Context compression.** Replaces a range of messages with a summary to save tokens.

```json
{
  "type": "Compaction",
  "id": "cmp_001",
  "compacted_range": ["msg_003", "msg_004", "msg_005"],
  "summary": "User asked about parser error handling...",
  "tokens_before": 2500,
  "tokens_after": 400,
  "timestamp": "2024-03-06T05:35:00Z"
}
```

### Branch

**Fork point.** Marks where a conversation split to explore alternatives.

```json
{
  "type": "Branch",
  "id": "branch_001",
  "from_message_id": "msg_006",
  "reason": "try recursive descent approach",
  "timestamp": "2024-03-06T05:40:00Z"
}
```

Fields:
- `id` — unique branch identifier
- `from_message_id` — the message where the fork occurred
- `reason` — human-readable reason for the fork
- `timestamp` — when the branch was created

### Label

**User-assigned label** for easy navigation.

```json
{
  "type": "Label",
  "id": "label_001",
  "target_message_id": "msg_010",
  "label": "working-parser",
  "timestamp": "2024-03-06T05:45:00Z"
}
```

Labels can be used with `/rewind` and `/switch` to jump to specific points.

### Custom

**Extensible entry** for application-specific metadata.

```json
{
  "type": "Custom",
  "id": "custom_001",
  "kind": "merge",
  "data": {
    "source_branch": "branch_001",
    "target_branch": "main",
    "strategy": "interactive",
    "merged_messages": ["msg_015", "msg_016"]
  },
  "timestamp": "2024-03-06T06:00:00Z"
}
```

The `kind` field discriminates different custom entry types:
- `"merge"` — branch merge metadata
- `"cherry-pick"` — cherry-pick operation metadata
- Any application-defined kind

### ModelChange

**Model switch mid-session.**

```json
{
  "type": "ModelChange",
  "id": "mc_001",
  "from_model": "claude-haiku-4-5",
  "to_model": "claude-sonnet-4-5",
  "timestamp": "2024-03-06T05:50:00Z"
}
```

### Resume

**Session resumed** after being closed.

```json
{
  "type": "Resume",
  "id": "resume_001",
  "resumed_at": "2024-03-06T12:00:00Z",
  "from_entry_id": "msg_020"
}
```

## Message Tree Structure

Messages form a **tree** via `parent_id` links:

```
msg_001 (root, parent_id: null)
  ├─ msg_002 (parent_id: msg_001)
  │   └─ msg_003 (parent_id: msg_002)
  │       ├─ msg_004 (parent_id: msg_003)  ← branch A
  │       └─ msg_005 (parent_id: msg_003)  ← branch B (fork point)
  └─ msg_006 (parent_id: msg_001)
```

**Key properties:**
- All branches share common ancestor prefixes (copy-on-write)
- Multiple messages can have the same `parent_id` (siblings = fork)
- Each branch is a path from a leaf to root
- All entries (all branches) live in the **same flat JSONL file**

## Reconstructing a Branch

Use `SessionTree::walk_branch(leaf_id)` to reconstruct the linear history for a specific branch:

```rust
let tree = SessionTree::build(entries);
let branch = tree.walk_branch(&leaf_id);
// Returns messages from root to leaf in order
```

The session tracks a `current_head` pointer indicating which leaf the user is viewing. Switching branches updates this pointer.

## Copy-on-Write Branching

When you fork:
1. Current message `msg_003` has child `msg_004`
2. User executes `/fork`
3. New message `msg_005` is created with `parent_id: msg_003`
4. Now `msg_003` has **two children**: `msg_004` and `msg_005`
5. The common prefix (`msg_001` → `msg_002` → `msg_003`) is shared

No duplication. Branches diverge only at the fork point.

## Example Session with Branching

```jsonl
{"type":"Header","session_id":"sess_001","created_at":"2024-03-06T05:00:00Z","cwd":"/project","model":"claude-sonnet-4-5","version":"0.1.0"}
{"type":"Message","id":"msg_001","parent_id":null,"message":{"type":"User","id":"msg_001","content":[{"type":"Text","text":"Help me write a parser"}],"timestamp":"2024-03-06T05:00:05Z"},"timestamp":"2024-03-06T05:00:05Z"}
{"type":"Message","id":"msg_002","parent_id":"msg_001","message":{"type":"Assistant","id":"msg_002","content":[{"type":"Text","text":"I can help with that. What kind of parser?"}],"model":"claude-sonnet-4-5","usage":{"input_tokens":50,"output_tokens":30},"stop_reason":"stop","timestamp":"2024-03-06T05:00:10Z"},"timestamp":"2024-03-06T05:00:10Z"}
{"type":"Message","id":"msg_003","parent_id":"msg_002","message":{"type":"User","id":"msg_003","content":[{"type":"Text","text":"A recursive descent parser"}],"timestamp":"2024-03-06T05:00:15Z"},"timestamp":"2024-03-06T05:00:15Z"}
{"type":"Message","id":"msg_004","parent_id":"msg_003","message":{"type":"Assistant","id":"msg_004","content":[{"type":"Text","text":"Here's a recursive descent approach..."}],"model":"claude-sonnet-4-5","usage":{"input_tokens":80,"output_tokens":200},"stop_reason":"stop","timestamp":"2024-03-06T05:00:25Z"},"timestamp":"2024-03-06T05:00:25Z"}
{"type":"Branch","id":"branch_001","from_message_id":"msg_003","reason":"try iterative approach instead","timestamp":"2024-03-06T05:01:00Z"}
{"type":"Message","id":"msg_005","parent_id":"msg_003","message":{"type":"Assistant","id":"msg_005","content":[{"type":"Text","text":"Let me show you an iterative parser..."}],"model":"claude-sonnet-4-5","usage":{"input_tokens":80,"output_tokens":180},"stop_reason":"stop","timestamp":"2024-03-06T05:01:10Z"},"timestamp":"2024-03-06T05:01:10Z"}
{"type":"Label","id":"label_001","target_message_id":"msg_003","label":"parser-fork-point","timestamp":"2024-03-06T05:02:00Z"}
{"type":"Custom","id":"custom_001","kind":"merge","data":{"source":"branch_001","target":"main","merged_ids":["msg_005"]},"timestamp":"2024-03-06T05:05:00Z"}
{"type":"Custom","id":"custom_002","kind":"cherry-pick","data":{"message_id":"msg_004","target":"branch_001"},"timestamp":"2024-03-06T05:10:00Z"}
```

## Storage and Performance

- **Append-only:** New entries are appended; old entries are never modified
- **O(1) message lookup:** `SessionTree` builds a hash index for instant access
- **O(depth) branch walk:** Walking from leaf to root is proportional to branch depth
- **Compact:** JSONL is space-efficient and human-readable
- **Git-friendly:** Text format diffs cleanly, good for version control

## Implementation Notes

See:
- `session/src/session/entry.rs` — entry type definitions
- `session/src/session/tree.rs` — tree traversal and query API
- `session/src/session/store.rs` — JSONL persistence
