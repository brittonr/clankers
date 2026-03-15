# Fork Command Specification

## Commands

### `/fork [reason]`

Create a new branch from the current message. The conversation forks at the
current point, allowing exploration of alternative approaches.

**Syntax:**
```
/fork [reason]
```

**Behavior:**
1. Emit `BranchEntry` to JSONL with `from_message_id = current_head` and optional `reason`
2. If `reason` is provided, use it as the branch name. Otherwise, generate `branch-<timestamp>`
3. Set `current_head` to `from_message_id` (rewind one step to the fork point)
4. User's next message will have `parent_id = from_message_id`, creating a sibling
5. Display confirmation: "Forked at message <id>. Branch: <name>"

**Examples:**
```
/fork try async version
/fork
```

**Edge cases:**
- If the session has zero messages, error: "Cannot fork: no messages yet"
- If already at a fork point (current message has siblings), proceed anyway
- Branch name collision: append counter (`try-async`, `try-async-2`)

---

### `/rewind <target>`

Jump back to an earlier message and continue from there. Creates an implicit fork.

**Syntax:**
```
/rewind <N>            # Go back N messages
/rewind <message-id>   # Jump to specific message by ID
/rewind <label>        # Jump to a labeled message
```

**Behavior:**
1. Resolve the target message (count, ID, or label)
2. Update `current_head` to the target message
3. Rebuild `Agent.messages` via `tree.walk_branch(target)`
4. Display confirmation: "Rewound to message <id> (<N> messages back)"
5. User's next message will have `parent_id = target`, creating a fork if the target already has children

**Examples:**
```
/rewind 5              # Go back 5 messages
/rewind msg-abc123     # Jump to specific message
/rewind #checkpoint    # Jump to labeled message
```

**Edge cases:**
- If N exceeds message count, clamp to root message
- If message-id doesn't exist, error with list of nearby IDs
- If rewinding to current head (no-op), confirm: "Already at message <id>"
- If rewinding to a non-leaf message, confirm: "Rewinding to <id>. This will create a new branch."

---

### `/branches`

List all conversation branches with metadata.

**Syntax:**
```
/branches [--verbose]
```

**Behavior:**
1. Walk the `SessionTree` to find all leaf messages
2. For each leaf, walk back to find the divergence point (where it split from siblings)
3. Display table with columns: Name, Current, Messages, Diverged At, Last Message Time
4. Highlight the active branch (where `current_head` is)
5. Optionally show branch tree ASCII art with `--verbose`

**Output:**
```
Branches:

  * main              (current)    42 messages    diverged at msg-15    5 minutes ago
    async-version                  38 messages    diverged at msg-30    2 hours ago
    builder-pattern                35 messages    diverged at msg-28    yesterday

  Use /switch <name> to change branches
```

**Edge cases:**
- If the session has only one linear branch, show: "No forks. Use /fork to explore alternatives."
- If verbose flag, render tree:
  ```
  msg-1 (root)
  ├─ msg-15 (main)
  │  └─ msg-42 *
  └─ msg-30 (async-version)
     └─ msg-38
  ```

---

### `/switch <branch-name|message-id>`

Switch to a different branch. Changes which conversation history the agent sees.

**Syntax:**
```
/switch <branch-name>
/switch <message-id>
```

**Behavior:**
1. Resolve branch name to a leaf message ID
2. Update `current_head` to the target leaf
3. Rebuild `Agent.messages` via `tree.walk_branch(current_head)`
4. Display confirmation: "Switched to branch <name> (message <id>)"
5. Show a summary of the branch: "This branch has N messages. Diverged from main at message M."

**Examples:**
```
/switch async-version
/switch msg-abc123
```

**Edge cases:**
- If branch name doesn't exist, error with list of available branches
- If switching to current branch (no-op), confirm: "Already on branch <name>"
- If switching to a non-leaf message, confirm: "Switched to <id>. This is not a leaf. New messages will continue from here."

---

### `/label <name>`

Add a human-readable label to the current message for easy navigation.

**Syntax:**
```
/label <name>
```

**Behavior:**
1. Emit `LabelEntry` to JSONL with `target_message_id = current_head` and `label = name`
2. Display confirmation: "Labeled message <id> as '<name>'"
3. Labels can be used in `/rewind` and `/switch` commands

**Examples:**
```
/label checkpoint
/label working-version
/label before-refactor
```

**Edge cases:**
- If label already exists on this message, overwrite with new label
- If label exists on another message, allow duplicate (labels are not unique)
- Labels are case-insensitive for lookups

---

## Implementation Notes

### Resolving targets

When a command takes a target (message-id, branch name, label), resolve in this order:

1. Exact message ID match
2. Label match (case-insensitive)
3. Branch name match (case-insensitive, fuzzy match if unique prefix)
4. Relative offset (e.g., `-5` for "5 messages back", `^` for parent)

### Branch naming strategy

- User-provided reason: slugify to `kebab-case`, truncate to 40 chars
- Auto-generated: `branch-<timestamp>` (e.g., `branch-2024-03-04-20:12`)
- Collision handling: append `-2`, `-3`, etc.

### Tracking the current head

The `Session` struct gains a new field:

```rust
pub struct Session {
    entries: Vec<SessionEntry>,
    tree: SessionTree,
    current_head: Option<MessageId>, // Active branch leaf
}
```

On session load, default to `tree.find_latest_leaf(None)` (most recent message in the most recent branch).

### Agent message loading

Before processing a user message, the Agent calls:

```rust
let messages = session.tree.walk_branch(&session.current_head.unwrap());
agent.set_messages(messages);
```

This replaces the current `session.messages()` flat list.

### Keyboard shortcuts

- `Ctrl+F` — Fork from current message (prompts for reason)
- `Ctrl+B` — Open branch switcher panel
- `Ctrl+R` — Rewind (prompts for count or message-id)
- `Ctrl+L` — Label current message (prompts for name)

### Slash command registration

All commands are registered in `src/tui/command_processor.rs` or equivalent.
Each command is a variant of `SlashCommand` enum with associated handler.
