# Branch Merge Specification

## Overview

Merging branches means copying messages from one branch into another, creating
a unified history. Unlike git, conversation merge is usually one-way: you take
the result from an exploratory branch and bring it back into the main branch.

There's no automatic conflict resolution — the user explicitly chooses which
messages to copy and where to place them.

---

## Commands

### `/merge <source> <target>`

Copy all messages from the source branch into the target branch.

**Syntax:**
```
/merge <source-branch> <target-branch>
```

**Behavior:**
1. Resolve source branch name to its leaf message ID
2. Walk the source branch from divergence point to leaf
3. Find the unique messages that exist only in the source branch (not shared ancestors)
4. Resolve target branch name to its leaf message ID
5. Append the source messages as children of the target leaf
6. Assign new MessageIds to the copied messages (preserve content, timestamp, role)
7. Emit a `BranchEntry` or `CustomEntry` to record the merge operation
8. Switch to the target branch (update `current_head` to the new leaf)
9. Display confirmation: "Merged N messages from <source> into <target>"

**Examples:**
```
/merge async-version main
/merge builder-pattern main
```

**Edge cases:**
- If source and target are the same branch, error: "Cannot merge branch into itself"
- If source branch has no unique messages (already merged or is an ancestor), warn: "No new messages to merge"
- If target branch doesn't exist, error with list of available branches

---

### `/merge-interactive <source> <target>`

Interactively choose which messages to merge.

**Syntax:**
```
/merge-interactive <source-branch> <target-branch>
```

**Behavior:**
1. Find unique messages in the source branch
2. Display a UI with checkboxes for each message
3. User selects which messages to include
4. Copy selected messages to target branch (preserving order)
5. Emit merge record to JSONL

**UI:**
```
╔═ Merge: async-version → main ════════════════════════╗
║                                                      ║
║  Select messages to merge:                           ║
║                                                      ║
║  [x] msg-31: [User] Use async version                ║
║  [x] msg-32: [Assistant] Here's async cache...       ║
║  [ ] msg-33: [User] Add timeout handling             ║
║  [x] msg-34: [Assistant] Added timeout logic...      ║
║                                                      ║
║  [Space] Toggle  [a] All  [n] None  [Enter] Merge    ║
╚══════════════════════════════════════════════════════╝
```

---

### `/cherry-pick <message-id> <target>`

Copy a single message (and optionally its children) from one branch to another.

**Syntax:**
```
/cherry-pick <message-id> <target-branch>
/cherry-pick <message-id> <target-branch> --with-children
```

**Behavior:**
1. Resolve message ID to a message in the tree
2. If `--with-children`, recursively collect all descendants
3. Resolve target branch to its leaf message ID
4. Append the message(s) as children of the target leaf
5. Assign new MessageIds
6. Display confirmation: "Cherry-picked message <id> into <target>"

**Examples:**
```
/cherry-pick msg-42 main
/cherry-pick msg-30 experimental --with-children
```

---

## Branch Comparison

Before merging, the user can compare branches to decide what to merge.

### `/compare <branch-a> <branch-b>`

Display a side-by-side diff of two branches.

**Syntax:**
```
/compare <branch-a> <branch-b>
```

**Behavior:**
1. Find the divergence point (last common ancestor)
2. Walk both branches from the divergence point to their leaves
3. Display a split-pane view showing unique messages in each branch

**UI:**

```
┌─ Compare: main vs async-version ─────────────────────┐
│                                                      │
│  Common ancestor: msg-30 (2 hours ago)               │
│  [User] Implement cache layer                        │
│                                                      │
│  ─ main (12 messages) ────┬─ async-version (8 msgs) │
│                           │                          │
│  msg-31: [User] Use LRU   │  msg-31: [User] Use      │
│           cache           │           async cache    │
│                           │                          │
│  msg-32: [Assistant]      │  msg-32: [Assistant]     │
│           Here's an LRU   │           Here's async   │
│           implementation  │           implementation │
│           with HashMap... │           with tokio...  │
│                           │                          │
│  [... 10 more messages]   │  [... 6 more messages]   │
│                           │                          │
│  [m] Merge ←  [c] Copy →  [q] Close                  │
└──────────────────────────────────────────────────────┘
```

**Keybindings:**
- `m` — Merge the right branch into the left
- `c` — Copy selected message to the other side
- `q` — Close comparison view
- `j`/`k` — Navigate messages
- `←`/`→` — Switch focus between panes

---

## Merge Strategies

### Full merge (default)

Copy all unique messages from source to target, preserving order.

```
Before:
  main: msg-1 → msg-2 → msg-3
  async: msg-1 → msg-2 → msg-4 → msg-5

After /merge async main:
  main: msg-1 → msg-2 → msg-3 → msg-4' → msg-5'
```

### Selective merge (interactive)

User chooses which messages to include.

```
Before:
  main: msg-1 → msg-2 → msg-3
  async: msg-1 → msg-2 → msg-4 → msg-5 → msg-6

After /merge-interactive async main (selected msg-4, msg-6):
  main: msg-1 → msg-2 → msg-3 → msg-4' → msg-6'
```

### Cherry-pick (single message)

Copy one message without its siblings.

```
Before:
  main: msg-1 → msg-2 → msg-3
  feature: msg-1 → msg-2 → msg-10 → msg-11

After /cherry-pick msg-10 main:
  main: msg-1 → msg-2 → msg-3 → msg-10'
```

---

## LLM-Assisted Merge (Future Work)

For complex branches where both sides have valuable content, an LLM can synthesize
a merged result.

### `/merge-llm <branch-a> <branch-b>`

**Behavior:**
1. Compare branches to find divergence point and unique messages
2. Send both conversation paths to the LLM with a merge prompt:
   ```
   "I have two conversation branches exploring different approaches.
   Branch A tried X, Branch B tried Y. Please synthesize a response
   that incorporates the best ideas from both."
   ```
3. LLM generates a new assistant message combining both approaches
4. Append the synthesized message to the target branch
5. Mark the merge with metadata (source branches, merge strategy)

**Out of scope for this change:** LLM-assisted merge is a future enhancement.
The infrastructure (branch comparison, message copying) is built first.

---

## Merge Metadata

When a merge occurs, record metadata in the JSONL file:

### Using CustomEntry

```json
{
  "type": "Custom",
  "id": "merge-abc123",
  "kind": "merge",
  "data": {
    "source_branch": "async-version",
    "source_leaf_id": "msg-38",
    "target_branch": "main",
    "target_leaf_id": "msg-42",
    "merged_messages": ["msg-31", "msg-32", "msg-34"],
    "strategy": "full" // or "selective", "cherry-pick", "llm"
  },
  "timestamp": "2024-03-04T20:30:00Z"
}
```

### Or using BranchEntry with extended reason

```json
{
  "type": "Branch",
  "id": "merge-abc123",
  "from_message_id": "msg-42",
  "reason": "merged async-version (8 messages)",
  "timestamp": "2024-03-04T20:30:00Z"
}
```

---

## UI for Merge Operations

### Branch panel merge action

From the branch panel, select a branch and press `m` to merge:

1. Prompt for target branch (autocomplete list)
2. Show preview of what will be merged (message count, divergence point)
3. Confirm merge
4. Execute merge and switch to target branch

### Compare view merge action

From the compare view, press `m` to merge the focused branch into the other:

1. Confirm which direction (left → right or right → left)
2. Show preview of merge
3. Execute merge and update the view

### Message context menu

Right-click on a message in the message view:

- "Cherry-pick to branch..." — prompts for target branch
- "Copy to main" — quick cherry-pick to main branch
- "Compare with..." — opens compare view starting from this message

---

## Edge Cases

### Merging already-merged branches

If source and target share all messages (no unique messages in source), warn:
```
No new messages to merge. Branch "async-version" is already merged or is an ancestor of "main".
```

### Circular merges

If branch A is an ancestor of branch B, merging B into A is a no-op (B already contains A).
If A and B are sibling branches, merging either way is valid.

### Merging with compactions

If the source branch has compacted messages (CompactionEntry), the compaction summary
is treated as a single message during merge. The target branch gets the summary, not
the original uncompacted messages.

### Message ID collisions

Merged messages are assigned new MessageIds to avoid collisions. The original message
ID is preserved in metadata for reference.

### Merge conflicts (content overlap)

If both branches have messages addressing the same user prompt, the merge duplicates them.
The user can manually delete unwanted messages after merge or use interactive merge to
filter them.

---

## Implementation Notes

### Finding unique messages in a branch

```rust
fn find_unique_messages(tree: &SessionTree, source_leaf: &MessageId, target_leaf: &MessageId) -> Vec<&MessageEntry> {
    let source_path = tree.walk_branch(source_leaf);
    let target_path = tree.walk_branch(target_leaf);
    
    // Find the divergence point (last common ancestor)
    let divergence_idx = source_path.iter().zip(target_path.iter())
        .position(|(a, b)| a.id != b.id)
        .unwrap_or(source_path.len().min(target_path.len()));
    
    // Messages unique to source are those after divergence
    source_path[divergence_idx..].to_vec()
}
```

### Copying messages with new IDs

```rust
fn copy_messages(messages: Vec<&MessageEntry>, parent_id: MessageId) -> Vec<MessageEntry> {
    let mut copied = Vec::new();
    let mut parent = parent_id;
    
    for msg in messages {
        let new_id = MessageId::new_v4();
        let new_msg = MessageEntry {
            id: new_id.clone(),
            parent_id: Some(parent.clone()),
            message: msg.message.clone(),
            timestamp: Utc::now(),
        };
        copied.push(new_msg);
        parent = new_id;
    }
    
    copied
}
```

### Emitting merge metadata

```rust
fn emit_merge_entry(session: &mut Session, source: String, target: String, merged_ids: Vec<MessageId>) {
    let entry = SessionEntry::Custom(CustomEntry {
        id: MessageId::new_v4(),
        kind: "merge".to_string(),
        data: json!({
            "source_branch": source,
            "target_branch": target,
            "merged_messages": merged_ids,
            "strategy": "full",
        }),
        timestamp: Utc::now(),
    });
    
    session.append_entry(entry);
}
```
