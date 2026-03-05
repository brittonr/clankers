# session-forking — Branch Conversations to Explore Alternatives

## Intent

Conversations with AI agents are iterative and exploratory. You often want to
try different approaches: "What if I asked the question differently?" or "Let
me rewind before that mistake and try a different path." The codebase already
has a complete message tree infrastructure — `SessionTree` can walk branches,
find children, and navigate the DAG — but none of it is exposed to users.

Right now, every session is strictly linear. Once a message is sent, you can't
go back and fork from an earlier point. The only workaround is to manually
edit the JSONL file or start a new session, losing all the context.

This change closes that gap: it exposes the existing tree structure through
user-facing commands and UI so you can:
- Fork the conversation from any message to explore an alternative
- Rewind to an earlier point and continue from there
- See which branches exist and navigate between them
- Compare different conversation paths side-by-side
- Merge successful explorations back into the main branch

## Scope

### In Scope

- `/fork [reason]` command — create a new branch from the current message
- `/rewind <message-id|N>` command — jump back N messages and continue from there
- `/branches` command — list all branches with metadata (names, creation time, message count)
- Branch visualization in the message view — indicators showing where branches diverge
- Branch switcher UI — navigate between branches in the TUI
- Branch naming and labeling (auto-generated or user-specified)
- Branch comparison tool — side-by-side diff of two conversation paths
- Branch merge — copy a message subtree from one branch to another
- Agent's `messages` array populated via `SessionTree::walk_branch(current_leaf)`
- Branch metadata storage in JSONL (`BranchEntry` already exists)
- Keyboard shortcuts for branch operations (Ctrl+F to fork, Ctrl+B to switch)

### Out of Scope

- Git worktree per branch (worktrees are for parallel agent execution, not conversation branching)
- Automatic branch creation on tool errors (future: auto-fork on failure for retry)
- LLM-assisted branch summarization (future: AI-generated branch descriptions)
- Branch garbage collection (orphaned branches, pruning old explorations)
- Multi-way merge conflict resolution (initial merge is manual pick)
- Undo/redo (branch rewinding is sufficient for now)
- Session-level branching (forking entire sessions, not individual conversations)

## Approach

The `SessionTree` already exists and parses the message DAG from JSONL entries.
The `Agent` struct currently uses `session.messages()` which returns a flat
linear history. We change this to:

1. Track a "current branch head" (a MessageId representing where the user is)
2. Use `SessionTree::walk_branch(current_head)` to populate `Agent.messages`
3. When the user sends a message, set its `parent_id` to the current head
4. When the user forks, emit a `BranchEntry` to JSONL and update the head
5. When the user switches branches, update the head and rebuild Agent.messages

The TUI gains:
- A branch indicator in the message list (show when a message has multiple children)
- A branch panel listing all leaf branches with metadata
- Keyboard shortcuts to fork, switch, and compare

The JSONL format already supports `BranchEntry`. We just need to emit them
at fork points and use them to populate the branch panel metadata.

No changes to the storage format. No changes to the message parent/child
relationships. Just wiring the tree navigation into the user interface and
Agent message loading.
