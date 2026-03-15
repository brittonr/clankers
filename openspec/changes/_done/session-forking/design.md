# session-forking — Design

## Decisions

### Copy-on-write branch semantics, not full copy

**Choice:** Branches share the common ancestor prefix. Only divergent messages are new.
**Rationale:** The tree structure already exists — each message has a `parent_id`.
Forking is just creating a new message with the same parent as the current message,
creating a sibling. No data duplication. The JSONL file remains a flat log of all
messages across all branches. `SessionTree::walk_branch(leaf_id)` reconstructs the
linear history for any branch on demand.
**Alternatives considered:** Full copy per branch (wasteful, breaks the DAG model),
git-style refs (adds complexity, the MessageId already serves as a stable ref).

### Track "current branch head" as a MessageId in session state

**Choice:** Store `current_head: MessageId` in the session. This is the leaf message
the user is currently viewing. When a new message is sent, its `parent_id` is set to
`current_head`. When the user switches branches, update `current_head` and rebuild
Agent.messages.
**Rationale:** Simple, stateless. The JSONL file is the source of truth. The head
is just a pointer for navigation. On session load, default to `find_latest_leaf(None)`.
**Alternatives considered:** Track branch names/labels as first-class refs (complex,
requires new JSONL entry type), store head in a separate state file (fragile, can
desync from JSONL).

### No git worktrees per branch

**Choice:** Worktrees remain for parallel agent sessions, not for conversation branches.
All branches of a conversation share the same filesystem state.
**Rationale:** Conversation branches are about exploring different prompts or reasoning
paths. The agent's filesystem view (files, git repo) doesn't change. Worktrees are for
running multiple agents in parallel on different tasks, not for forking conversations.
**Alternatives considered:** One worktree per branch (overcomplicated, worktrees are
heavy, would require syncing filesystem state between branches).

### Branch indicators in linear message view, not a tree visualization

**Choice:** Message list shows branch points with indicators like `├─ 2 branches` at
divergence points. Message IDs are visible for reference. No graphical tree rendering.
**Rationale:** Most conversations are mostly linear with occasional forks. A full tree
view (like `git log --graph`) is visually noisy for deep conversations. Indicators at
branch points are sufficient. Users can open a branch panel to see all branches.
**Alternatives considered:** Full tree ASCII art (cluttered, hard to read long messages),
separate tree panel always visible (wastes screen space for linear sessions).

### Auto-generated branch names from fork reason or timestamp

**Choice:** When forking, if the user provides a reason (`/fork try another approach`),
use that as the branch name. Otherwise, generate a name like `branch-2024-03-04-20:12`.
BranchEntry stores both `reason` (user-provided) and `timestamp`.
**Rationale:** Branches need identifiable labels for the switcher UI. User-provided
reasons are descriptive. Timestamps are unique and sortable when no reason given.
**Alternatives considered:** Numeric IDs (`branch-1`, `branch-2`) — not descriptive,
UUIDs — not human-friendly.

### Manual branch merge via message copy

**Choice:** Merging means copying a message subtree from one branch to another.
`/merge <source-branch> <target-branch>` finds the source leaf, walks its tree,
and appends those messages as children of the target leaf with new MessageIds.
**Rationale:** Simple and explicit. The user decides which branch wins. No automatic
conflict resolution. LLM-assisted merge (comparing two branches and synthesizing a
result) is future work.
**Alternatives considered:** Automatic merge (ambiguous, what if both branches added
messages?), LLM-based merge (complex, needs a new agent interaction model), git-style
three-way merge (overkill for conversation trees).

### Agent.messages populated via SessionTree::walk_branch

**Choice:** When loading Agent messages, instead of `session.messages()` (flat linear),
call `session_tree.walk_branch(current_head)` to get the active branch's history.
**Rationale:** This is the core change. The Agent sees a linear conversation, but it's
actually a slice through the tree. Switching branches just changes which slice is active.
**Alternatives considered:** Always load all messages and filter by ancestry (wasteful,
confuses the agent with multiple branches in context), rebuild Agent.messages only on
branch switch (works but requires invalidation logic).

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Session / Agent                         │
│                                                             │
│  ┌────────────────────────────────────────────────────┐     │
│  │  Session                                           │     │
│  │                                                    │     │
│  │  entries: Vec<SessionEntry>  (all branches)        │     │
│  │  tree: SessionTree           (DAG index)           │     │
│  │  current_head: MessageId     (active branch leaf)  │     │
│  └──────────────────┬─────────────────────────────────┘     │
│                     │                                       │
│                     │ walk_branch(current_head)             │
│                     ▼                                       │
│  ┌────────────────────────────────────────────────────┐     │
│  │  Agent                                             │     │
│  │                                                    │     │
│  │  messages: Vec<Message>  (current branch history)  │     │
│  └────────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────┘

           ┌───────────────────────────────────────┐
           │         User Commands                 │
           │                                       │
           │  /fork [reason]  → create new branch  │
           │  /rewind N       → jump back N msgs   │
           │  /branches       → list all branches  │
           │  /switch <name>  → change active br   │
           │  /compare A B    → diff two branches  │
           │  /merge A → B    → copy subtree       │
           └───────────────┬───────────────────────┘
                           │
                           ▼
           ┌───────────────────────────────────────┐
           │         TUI Components                │
           │                                       │
           │  MessageView:                         │
           │    - branch indicators at divergence  │
           │    - show message IDs for reference   │
           │                                       │
           │  BranchPanel:                         │
           │    - list all leaf branches           │
           │    - metadata: name, time, msg count  │
           │    - highlight active branch          │
           │                                       │
           │  BranchCompareView:                   │
           │    - side-by-side message diff        │
           │    - show divergence point            │
           └───────────────────────────────────────┘
```

## Data Flow

### Forking from current message

1. User sends message "implement feature X"
2. Agent responds with code
3. User decides to try a different approach: `/fork try builder pattern instead`
4. Session emits `BranchEntry { id: new_id, from_message_id: current_head, reason: "try builder pattern instead" }`
5. Session sets `current_head` to the `from_message_id` (rewinds one step)
6. User sends new message "use builder pattern"
7. New message's `parent_id` is set to the fork point
8. Both branches now exist: original agent response and the new builder pattern path

### Switching between branches

1. User in main branch (leaf message ID `msg-42`)
2. User calls `/branches` — sees "main (current)", "builder-pattern", "async-version"
3. User calls `/switch builder-pattern` (or uses Ctrl+B shortcut)
4. Session finds the leaf message of the builder-pattern branch (walk from fork point)
5. Session updates `current_head` to that leaf's MessageId
6. Agent.messages is rebuilt via `tree.walk_branch(current_head)`
7. Agent now sees the builder-pattern conversation history

### Rewinding without explicit fork

1. User at message 50 in a linear conversation
2. User realizes the conversation went off track at message 45
3. User calls `/rewind 5` (or `/rewind msg-45`)
4. Session updates `current_head` to message 45
5. Agent.messages is rebuilt via `tree.walk_branch(msg-45)`
6. User sends new message — it becomes a sibling of message 46
7. Implicit fork created (no BranchEntry emitted unless user explicitly names it)

### Comparing branches

1. User has two branches: "sync-approach" and "async-approach"
2. User calls `/compare sync-approach async-approach`
3. Session finds the divergence point (last common ancestor)
4. TUI renders side-by-side view:
   - Left pane: messages unique to sync-approach
   - Right pane: messages unique to async-approach
   - Top: common ancestor context
5. User can navigate, copy messages, or decide which branch to continue

### Merging branches

1. User likes the async-approach result but wants to bring it into main
2. User calls `/merge async-approach main`
3. Session finds the leaf of async-approach
4. Session walks the tree to get the unique messages in async-approach
5. Session appends those messages as children of main's leaf (new MessageIds, preserving content)
6. BranchEntry emitted to record the merge operation
7. Main branch now includes the async work

## Storage Format

No changes to the JSONL schema. `BranchEntry` already exists:

```json
{
  "type": "Branch",
  "id": "branch-abc123",
  "from_message_id": "msg-42",
  "reason": "try builder pattern instead",
  "timestamp": "2024-03-04T20:12:34Z"
}
```

This is emitted when a user explicitly forks with `/fork`. Implicit forks
(rewinding and continuing) don't emit BranchEntry unless the user names the branch.

Branch metadata (names, descriptions) can be stored as `LabelEntry` targeting
the branch point message or the leaf message. Labels are already supported in
the format.

## Edge Cases

### Forking from the root (no messages yet)

If the session has zero messages and the user calls `/fork`, treat it as a no-op
or emit a warning. Forking requires at least one message to branch from.

### Switching to a branch that doesn't exist

If the user calls `/switch nonexistent-branch`, list available branches and error.
The TUI branch panel provides autocomplete/selection to avoid typos.

### Rewinding past the beginning

If the user calls `/rewind 1000` in a 50-message session, clamp to the root message.
Alternatively, error and show the message count.

### Deleting a branch

Out of scope for this change. Future: `/delete-branch <name>` removes BranchEntry
and optionally garbage-collects orphaned messages. For now, branches persist forever.

### Branch names colliding

Auto-generated names use timestamps (unique) or user-provided reasons (potentially
ambiguous). If a collision occurs, append a counter: "try-async", "try-async-2".

### Compaction and branches

Compaction replaces a range of messages with a summary. If that range is part of
multiple branches, the summary applies to all branches. SessionTree handles this
by treating CompactionEntry as a virtual message in the tree. Branch switching
remains unaffected.
