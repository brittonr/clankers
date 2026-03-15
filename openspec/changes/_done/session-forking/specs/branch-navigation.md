# Branch Navigation Specification

## TUI Components

### Branch Indicators in Message View

The message list shows visual indicators when a message has multiple children
(is a branch point).

**Rendering:**

```
┌─ Messages ───────────────────────────────────────────┐
│                                                      │
│ [User] Implement the cache layer                     │
│ [Assistant] Here's a simple HashMap-based cache...   │
│                                                      │
│ ├─ 2 branches                                        │  ← Indicator
│ │                                                    │
│ ├─ [User] Use an LRU cache instead  (main) *        │  ← Active branch
│ │  [Assistant] Here's an LRU implementation...      │
│ │                                                    │
│ └─ [User] Try async cache (async-version)           │  ← Alternate branch
│    [Assistant] Here's an async cache...             │
│                                                      │
└──────────────────────────────────────────────────────┘
```

**Behavior:**
- When a message has multiple children, show `├─ N branches`
- Indent child messages with `├─` or `└─` tree characters
- Highlight the active branch with `*` marker
- Show branch names if available (from BranchEntry reason or labels)
- Dim inactive branches (gray text)

**Limitations:**
- Only show branch indicators for direct children, not full tree recursion
- Deep branching (3+ levels) shows "N more branches" link
- Click to expand collapsed branches

---

### Branch Panel

A dedicated panel for viewing and switching between all branches in the session.

**Layout:**

```
┌─ Branches ───────────────────────────────────────────┐
│                                                      │
│  * main                                              │  ← Active
│    42 messages                                       │
│    Last: 5 minutes ago                               │
│    "Implement cache layer with LRU"                  │
│                                                      │
│    async-version                                     │
│    38 messages                                       │
│    Last: 2 hours ago                                 │
│    "Try async cache implementation"                  │
│                                                      │
│    builder-pattern                                   │
│    35 messages                                       │
│    Last: yesterday                                   │
│    "Use builder pattern for cache config"            │
│                                                      │
│  [Enter] Switch  [d] Details  [c] Compare  [m] Merge │
└──────────────────────────────────────────────────────┘
```

**Behavior:**
- List all leaf branches (one per fork)
- Show branch name (from BranchEntry reason, label, or auto-generated)
- Show message count in the branch
- Show last activity timestamp
- Show the most recent user message as preview
- Highlight the active branch with `*`

**Keybindings:**
- `Enter` — Switch to selected branch
- `d` — Show branch details (full tree, divergence point, message list)
- `c` — Compare selected branch with another (prompts for second branch)
- `m` — Merge selected branch into another (prompts for target)
- `j`/`k` or arrow keys — Navigate branch list
- `q` or `Esc` — Close panel

**Opening the panel:**
- Slash command: `/branches`
- Keyboard shortcut: `Ctrl+B`
- Menu item: "View > Branches"

---

### Branch Switcher (Quick Picker)

A lightweight overlay for quickly switching branches without opening the full panel.

**Layout:**

```
╔═ Switch Branch ══════════════════════════════════════╗
║                                                      ║
║  > main (current)                        42 messages ║
║    async-version                         38 messages ║
║    builder-pattern                       35 messages ║
║                                                      ║
║  Type to filter...                                   ║
╚══════════════════════════════════════════════════════╝
```

**Behavior:**
- Show a floating overlay with all branches
- Type-ahead filtering by branch name
- Fuzzy matching (e.g., "async" matches "async-version")
- `Enter` to switch, `Esc` to cancel
- Up/down arrows to navigate

**Opening the switcher:**
- Keyboard shortcut: `Ctrl+Shift+B`
- Start typing a branch name in the command input (autocomplete)

---

### Branch Details View

A detailed view of a single branch showing its full history and metadata.

**Layout:**

```
┌─ Branch: async-version ──────────────────────────────┐
│                                                      │
│  Created: 2 hours ago                                │
│  Diverged from: main at message msg-30               │
│  Messages: 38                                        │
│  Last activity: 10 minutes ago                       │
│                                                      │
│  Reason: "Try async cache implementation"            │
│                                                      │
│  ─ Divergence Point ─────────────────────────────    │
│  msg-30: [User] Implement cache layer                │
│                                                      │
│  ─ Branch Messages ──────────────────────────────    │
│  msg-31: [User] Use async version                    │
│  msg-32: [Assistant] Here's an async cache...        │
│  msg-33: [User] Add timeout handling                 │
│  ...                                                 │
│                                                      │
│  [s] Switch to this branch  [c] Compare  [m] Merge   │
└──────────────────────────────────────────────────────┘
```

**Behavior:**
- Show full metadata for the branch
- Show the divergence point (where it split from parent)
- Show a scrollable list of all messages in the branch
- Provide actions: switch, compare, merge

**Opening the details view:**
- From branch panel: press `d` on a branch
- From branch indicator in message view: click on branch name
- Slash command: `/branch-details <name>`

---

### Message ID Display

Each message shows its ID for reference in commands like `/rewind` and `/switch`.

**Rendering:**

```
┌─ Messages ───────────────────────────────────────────┐
│                                                      │
│ msg-42 [User] Implement cache layer                  │
│ msg-43 [Assistant] Here's a cache implementation...  │
│                                                      │
└──────────────────────────────────────────────────────┘
```

**Behavior:**
- Show message ID as a prefix in dim text (gray)
- Copy message ID to clipboard on click or hover action
- Highlight message ID when referenced in commands

**Configuration:**
- Toggle message ID visibility with `Ctrl+I` or config setting
- Default: hidden for clean UI, shown on hover

---

## Keyboard Shortcuts

### Global shortcuts (available in message view)

| Key           | Action                          |
|---------------|---------------------------------|
| `Ctrl+F`      | Fork from current message       |
| `Ctrl+B`      | Open branch panel               |
| `Ctrl+Shift+B`| Open branch switcher (quick)    |
| `Ctrl+R`      | Rewind (prompts for target)     |
| `Ctrl+L`      | Label current message           |
| `Ctrl+I`      | Toggle message ID display       |

### Branch panel shortcuts

| Key      | Action                           |
|----------|----------------------------------|
| `Enter`  | Switch to selected branch        |
| `d`      | Show branch details              |
| `c`      | Compare with another branch      |
| `m`      | Merge into another branch        |
| `j`/`k`  | Navigate up/down                 |
| `/`      | Filter branches (type-ahead)     |
| `q`/`Esc`| Close panel                      |

### Branch switcher shortcuts

| Key      | Action                           |
|----------|----------------------------------|
| `Enter`  | Switch to selected branch        |
| `Esc`    | Cancel                           |
| `↑`/`↓`  | Navigate                         |
| Type     | Filter branches (fuzzy)          |

---

## Branch Metadata

Branches can have metadata to make them identifiable:

### Branch name
- From BranchEntry `reason` field (user-provided with `/fork reason`)
- From LabelEntry targeting the branch point or leaf
- Auto-generated: `branch-<timestamp>`

### Branch description
- Stored as a label or custom entry
- Displayed in branch panel and details view

### Branch statistics
- Message count: walk from divergence point to leaf
- Last activity: timestamp of most recent message in branch
- Divergence point: parent message where branch split

---

## Implementation Notes

### Detecting branch points

A message is a branch point if it has multiple children:

```rust
fn is_branch_point(tree: &SessionTree, msg_id: &MessageId) -> bool {
    tree.get_children(&Some(msg_id.clone())).len() > 1
}
```

### Finding all branches

Walk the tree to find all leaf messages. Each leaf represents a branch:

```rust
fn find_all_branches(tree: &SessionTree) -> Vec<BranchInfo> {
    // Start from roots (messages with parent_id = None)
    let roots = tree.get_children(&None);
    let mut leaves = Vec::new();
    
    // DFS to find all leaves
    fn walk(tree: &SessionTree, msg: &MessageEntry, leaves: &mut Vec<MessageId>) {
        let children = tree.get_children(&Some(msg.id.clone()));
        if children.is_empty() {
            leaves.push(msg.id.clone());
        } else {
            for child in children {
                walk(tree, child, leaves);
            }
        }
    }
    
    for root in roots {
        walk(tree, root, &mut leaves);
    }
    
    leaves.into_iter().map(|leaf_id| {
        BranchInfo {
            leaf_id: leaf_id.clone(),
            name: resolve_branch_name(tree, &leaf_id),
            message_count: tree.walk_branch(&leaf_id).len(),
            last_activity: tree.find_message_public(&leaf_id).unwrap().timestamp,
        }
    }).collect()
}
```

### Rendering branch indicators

In the message view, after rendering each message, check if it's a branch point:

```rust
fn render_message(msg: &MessageEntry, tree: &SessionTree, current_head: &MessageId) -> String {
    let mut output = format!("{} [{}] {}", msg.id, msg.role, msg.content);
    
    let children = tree.get_children(&Some(msg.id.clone()));
    if children.len() > 1 {
        output.push_str(&format!("\n├─ {} branches", children.len()));
        for child in children {
            let is_active = tree.walk_branch(current_head).iter().any(|m| m.id == child.id);
            let marker = if is_active { "*" } else { "" };
            let branch_name = resolve_branch_name(tree, &child.id);
            output.push_str(&format!("\n├─ {} ({}){}", child.content_preview(), branch_name, marker));
        }
    }
    
    output
}
```

### Branch name resolution

Resolve a branch name from BranchEntry, LabelEntry, or fallback to generated name:

```rust
fn resolve_branch_name(tree: &SessionTree, leaf_id: &MessageId) -> String {
    // 1. Check for BranchEntry at the divergence point
    // 2. Check for LabelEntry targeting the leaf or divergence point
    // 3. Fallback to "branch-<timestamp>"
}
```
