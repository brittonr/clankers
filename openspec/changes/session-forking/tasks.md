# session-forking — Tasks

## Phase 1: Core state management (no UI, no commands)

- [ ] Add `current_head: Option<MessageId>` field to `Session` struct
- [ ] Implement `Session::set_current_head(message_id)` — updates head and validates it exists
- [ ] Implement `Session::get_current_head()` — returns current head, defaults to `tree.find_latest_leaf(None)`
- [ ] Modify `Session::load()` to initialize `current_head` to latest leaf on load
- [ ] Modify `Agent::load_messages()` to use `tree.walk_branch(current_head)` instead of flat `messages()`
- [ ] Ensure new messages set `parent_id` to `current_head` when sent
- [ ] Unit tests: current head tracking, branch walking, message parent linkage

## Phase 2: Fork and rewind commands

- [ ] Implement `/fork [reason]` command in `src/tui/command_processor.rs`
  - [ ] Emit `BranchEntry` to JSONL with `from_message_id = current_head`
  - [ ] Generate branch name from reason or timestamp
  - [ ] Handle name collisions (append counter)
  - [ ] Update `current_head` to fork point (rewind one step)
  - [ ] Display confirmation message
- [ ] Implement `/rewind <target>` command
  - [ ] Parse target: numeric offset, message-id, or label
  - [ ] Resolve target to a MessageId
  - [ ] Update `current_head` to target
  - [ ] Rebuild `Agent.messages` via `tree.walk_branch(target)`
  - [ ] Display confirmation with message count
- [ ] Implement `/label <name>` command
  - [ ] Emit `LabelEntry` to JSONL targeting `current_head`
  - [ ] Store label → message-id mapping in session
- [ ] Unit tests: fork creates BranchEntry, rewind updates head, labels persist

## Phase 3: Branch listing and switching

- [ ] Implement `SessionTree::find_all_leaves()` — returns all leaf message IDs
- [ ] Implement `Session::find_branches()` — walks tree to build BranchInfo list
- [ ] Add `BranchInfo` struct: `{ leaf_id, name, message_count, last_activity, divergence_point }`
- [ ] Implement branch name resolution logic:
  - [ ] Check BranchEntry `reason` field at divergence point
  - [ ] Check LabelEntry targeting leaf or divergence
  - [ ] Fallback to `branch-<timestamp>`
- [ ] Implement `/branches [--verbose]` command
  - [ ] List all branches with metadata
  - [ ] Highlight active branch (current_head)
  - [ ] Optional: render ASCII tree with `--verbose`
- [ ] Implement `/switch <branch-name|message-id>` command
  - [ ] Resolve branch name to leaf message ID
  - [ ] Update `current_head` to target leaf
  - [ ] Rebuild `Agent.messages`
  - [ ] Display confirmation with branch summary
- [ ] Unit tests: branch discovery, name resolution, switching

## Phase 4: Branch indicators in message view

- [ ] Add `SessionTree::is_branch_point(message_id)` helper
- [ ] Modify message view renderer to detect branch points
- [ ] Render `├─ N branches` indicator when message has multiple children
- [ ] Indent child messages with tree characters (`├─`, `└─`)
- [ ] Highlight active branch with `*` marker
- [ ] Show branch names for child branches
- [ ] Dim inactive branches (gray text)
- [ ] Add config option to toggle message ID display (`Ctrl+I`)
- [ ] Integration test: verify branch indicators render correctly

## Phase 5: Branch panel (TUI component)

- [ ] Create `src/tui/components/branch_panel.rs` implementing `Panel` trait
- [ ] Implement branch list view:
  - [ ] Fetch all branches via `Session::find_branches()`
  - [ ] Display table: name, message count, last activity, preview
  - [ ] Highlight active branch with `*`
  - [ ] Show divergence point for each branch
- [ ] Implement keybindings:
  - [ ] `Enter` — switch to selected branch
  - [ ] `d` — show branch details view
  - [ ] `c` — compare with another branch (prompts)
  - [ ] `m` — merge into another branch (prompts)
  - [ ] `j`/`k` — navigate list
  - [ ] `q`/`Esc` — close panel
- [ ] Register panel in `src/tui/components/mod.rs`
- [ ] Add keyboard shortcut `Ctrl+B` to open branch panel
- [ ] Integration test: open panel, navigate, switch branches

## Phase 6: Branch details view

- [ ] Create `BranchDetailsView` component
- [ ] Implement layout:
  - [ ] Show branch metadata (created, diverged from, message count, last activity)
  - [ ] Show divergence point message
  - [ ] Show scrollable list of branch messages
  - [ ] Provide actions: switch, compare, merge
- [ ] Add `/branch-details <name>` command
- [ ] Keybindings: `s` (switch), `c` (compare), `m` (merge), `q` (close)
- [ ] Integration test: view details, navigate messages

## Phase 7: Branch switcher (quick picker)

- [ ] Create `BranchSwitcher` overlay component
- [ ] Implement floating list with all branches
- [ ] Add type-ahead filtering (fuzzy match)
- [ ] Keybindings: `Enter` (switch), `Esc` (cancel), `↑`/`↓` (navigate)
- [ ] Add keyboard shortcut `Ctrl+Shift+B` to open switcher
- [ ] Integration test: filter branches, select, switch

## Phase 8: Branch comparison

- [ ] Implement `Session::compare_branches(branch_a, branch_b)` — returns comparison data
  - [ ] Find divergence point (last common ancestor)
  - [ ] Walk both branches from divergence to leaves
  - [ ] Collect unique messages for each branch
- [ ] Implement `/compare <branch-a> <branch-b>` command
  - [ ] Open comparison view with side-by-side layout
- [ ] Create `BranchCompareView` component
  - [ ] Split-pane layout (left: branch A, right: branch B)
  - [ ] Show divergence point at top
  - [ ] Show unique messages in each pane
  - [ ] Highlight current position in each branch
- [ ] Keybindings:
  - [ ] `m` — merge right into left
  - [ ] `c` — copy selected message to other side
  - [ ] `←`/`→` — switch focus between panes
  - [ ] `j`/`k` — navigate messages
  - [ ] `q` — close view
- [ ] Integration test: compare branches, navigate, trigger merge

## Phase 9: Branch merge (full merge)

- [ ] Implement `Session::find_unique_messages(source_leaf, target_leaf)` helper
- [ ] Implement `Session::merge_branch(source, target)` — full merge strategy
  - [ ] Find unique messages in source branch
  - [ ] Copy messages with new MessageIds
  - [ ] Append to target branch as children of target leaf
  - [ ] Emit merge metadata (CustomEntry or BranchEntry)
  - [ ] Update `current_head` to new target leaf
- [ ] Implement `/merge <source> <target>` command
- [ ] Display merge confirmation with message count
- [ ] Unit tests: merge messages, verify parent linkage, check metadata

## Phase 10: Interactive merge

- [ ] Create `MergeInteractiveView` component
  - [ ] List all unique messages in source branch
  - [ ] Checkboxes for each message (default: all selected)
  - [ ] Keybindings: `Space` (toggle), `a` (all), `n` (none), `Enter` (merge)
- [ ] Implement `/merge-interactive <source> <target>` command
- [ ] Implement selective merge logic (copy only selected messages)
- [ ] Integration test: select messages, merge, verify result

## Phase 11: Cherry-pick

- [ ] Implement `Session::cherry_pick(message_id, target_leaf, with_children)` helper
- [ ] Implement `/cherry-pick <message-id> <target> [--with-children]` command
- [ ] Copy single message (or subtree) to target branch
- [ ] Emit cherry-pick metadata
- [ ] Unit tests: cherry-pick single message, with children

## Phase 12: Keyboard shortcuts and polish

- [ ] Register global shortcuts:
  - [ ] `Ctrl+F` — fork from current message (prompts for reason)
  - [ ] `Ctrl+B` — open branch panel
  - [ ] `Ctrl+Shift+B` — open branch switcher
  - [ ] `Ctrl+R` — rewind (prompts for target)
  - [ ] `Ctrl+L` — label current message (prompts for name)
  - [ ] `Ctrl+I` — toggle message ID display
- [ ] Add shortcuts to keybindings documentation (`docs/keybindings.md`)
- [ ] Add branch commands to help text (`/help`)
- [ ] Integration test: verify all shortcuts work

## Phase 13: Edge cases and error handling

- [ ] Handle fork from empty session (error: no messages)
- [ ] Handle rewind past beginning (clamp to root or error)
- [ ] Handle switch to nonexistent branch (error with list)
- [ ] Handle merge of same branch (error)
- [ ] Handle merge with no unique messages (warn)
- [ ] Handle label collision (allow duplicates)
- [ ] Handle branch name collision (append counter)
- [ ] Unit tests for all edge cases

## Phase 14: Documentation and examples

- [ ] Update `docs/session-format.md` with branch examples
- [ ] Update `docs/commands.md` with new slash commands
- [ ] Add tutorial: "Branching conversations" to `docs/tutorials/`
- [ ] Add example session JSONL with branches to `examples/sessions/`
- [ ] Update README with branching feature mention

## Phase 15: Performance and optimization (optional)

- [ ] Benchmark `walk_branch` performance on large trees (1000+ messages)
- [ ] Optimize branch discovery (cache leaf list)
- [ ] Optimize branch name resolution (cache BranchEntry/LabelEntry mappings)
- [ ] Add lazy loading for branch panel (paginate if many branches)
- [ ] Profile memory usage with many branches

## Phase 16: Future enhancements (post-MVP)

- [ ] LLM-assisted merge (`/merge-llm <branch-a> <branch-b>`)
- [ ] Auto-fork on tool errors for retry
- [ ] Branch garbage collection (prune orphaned branches)
- [ ] Undo/redo for branch operations
- [ ] Session-level branching (fork entire sessions)
- [ ] Branch export (save branch as new session file)
- [ ] Branch templates (common fork patterns)
