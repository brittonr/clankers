# Branching Conversations Tutorial

Branching lets you explore multiple approaches without losing your work. This tutorial walks through a realistic scenario: implementing a parser with two different strategies.

## Scenario: Building a Parser

You're working on a JSON parser and want to try both recursive and iterative approaches.

## 1. Start with the Problem

Begin your conversation normally:

```
You: Help me implement a JSON parser in Rust
```

The assistant responds with some initial ideas and asks what approach you prefer.

```
You: Let's start with a recursive descent parser
```

The assistant implements a recursive version. You test it and it works, but you're curious about performance.

## 2. Fork to Try an Alternative

Now you want to explore an iterative approach **without losing the recursive version**. Use `/fork`:

```
/fork try iterative parser
```

What happened:
- A new branch was created from the current message
- You're now on the new branch
- The recursive parser discussion is preserved on the original branch
- Both branches share the common history before the fork point

## 3. Continue on the New Branch

Continue the conversation on the new branch:

```
You: Can you show me an iterative version using explicit stacks?
```

The assistant implements an iterative parser. Now you have two complete implementations in different branches.

## 4. Navigate Between Branches

View all branches:

```
/branches
```

Output:
```
Branches:
  main (4 messages)
  try-iterative-parser (3 messages after fork)

Current: try-iterative-parser
```

For a detailed tree view:

```
/branches --verbose
```

Switch back to the recursive version:

```
/switch main
```

You're now viewing the conversation with the recursive parser. Switch back:

```
/switch try-iterative-parser
```

**Keyboard shortcut:** Press `Shift+B` to open a fuzzy picker for quick branch switching.

## 5. Label Important Points

Mark the fork point for easy navigation:

```
/label parser-fork-point
```

Mark the recursive implementation:

```
/switch main
/label recursive-complete
```

Mark the iterative implementation:

```
/switch try-iterative-parser
/label iterative-complete
```

Now you can jump directly to any labeled point:

```
/rewind parser-fork-point
/rewind recursive-complete
```

## 6. Compare the Approaches

Open the branch panel to see both branches side-by-side:

**Keyboard shortcut:** Press `b` (normal mode) or `Ctrl+B` (insert mode)

Compare specific messages:

```
/compare msg_abc123 msg_def456
```

This shows a side-by-side diff of the two implementations.

## 7. Merge the Best Parts

After testing both, you decide the iterative version has better performance, but the recursive version has better error handling. Use interactive merge:

```
/merge-interactive try-iterative-parser main
```

This opens a menu where you can:
- Select which messages to include
- Choose which version's approach to keep
- Combine error handling from one branch with performance from another

Or merge everything from one branch:

```
/merge try-iterative-parser main
```

## 8. Cherry-Pick Specific Ideas

Maybe you only want the error handling code from the recursive version:

```
/switch try-iterative-parser
/cherry-pick msg_error_handling main
```

This copies just that message (and optionally its descendants with `--with-children`) to the target branch.

## 9. Working with Message IDs

Enable message ID display to see what you're working with:

**Keyboard shortcut:** Press `Shift+I` or `Ctrl+I`

Now each message shows its ID in the margin. Use these IDs with:
- `/rewind <message-id>`
- `/switch <message-id>`
- `/cherry-pick <message-id>`
- `/compare <id-a> <id-b>`

## Advanced: Multiple Forks

You can fork multiple times to create a tree of experiments:

```
You: Help with parser
Assistant: [suggests approaches]

/fork try-recursive
You: Implement recursive version
Assistant: [recursive implementation]

/rewind 2
/fork try-iterative  
You: Implement iterative version
Assistant: [iterative implementation]

/rewind 2
/fork try-combinator
You: Use parser combinators instead
Assistant: [combinator implementation]
```

Now you have three branches exploring different parsing strategies, all sharing the initial conversation.

```
/branches
```

Output:
```
Branches:
  main (2 messages)
  try-recursive (4 messages after fork)
  try-iterative (3 messages after fork)
  try-combinator (5 messages after fork)
```

## Tips and Tricks

### Quick Navigation
- `b` — toggle branch panel
- `Shift+B` — branch switcher (fuzzy search)
- `Shift+I` — show message IDs

### Branch Naming
- Use descriptive names: `/fork try-async-version`
- Or let clankers auto-name: `/fork`

### Labels as Checkpoints
Label important points for easy return:
```
/label before-refactor
/label working-tests
/label ready-for-review
```

### Rewind vs Switch
- `/rewind` — jump back in the **current branch**
- `/switch` — switch to a **different branch**

### Experimenting Safely
Fork before risky changes:
```
You: This looks good
/label stable-version
/fork try-major-refactor
You: Let's completely restructure this...
```

If the refactor fails, just `/switch stable-version`.

### Comparing Approaches
Use `/compare` to see differences:
```
/compare recursive-complete iterative-complete
```

### Cleaning Up
After merging, you might want to continue on the merged branch. Switch to it and continue the conversation.

## Real-World Example: API Redesign

```bash
# Start with current API
You: Review the current API design

# Fork to try breaking changes
/fork redesign-api-v2
You: Let's make this more ergonomic with a builder pattern

# Another fork for different approach  
/rewind 1
/fork redesign-api-fluent
You: How about a fluent interface?

# Label current stable version
/switch main
/label api-v1-stable

# Compare the redesigns
/compare redesign-api-v2 redesign-api-fluent

# Pick the winner and merge
/merge redesign-api-fluent main
You: Great! Now let's add documentation for the new API
```

## How It Works Under the Hood

Branching is **copy-on-write**:
- Messages before the fork point are shared by all branches
- Only messages after the fork are unique to each branch
- Everything lives in a single JSONL file
- No duplication until you diverge

See [`docs/session-format.md`](../session-format.md) for technical details.

## Next Steps

- Try `/fork` next time you want to explore alternatives
- Use `/label` to mark important points
- Use `/compare` to review different approaches
- Use `/merge` when you want to combine the best parts

Happy branching! 🌳
