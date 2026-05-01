Artifact-Type: module-inventory
Task-ID: inventory-existing-modules
Covers: r[checkpoints-rollback.capability], r[checkpoints-rollback.observability]
Generated: 2026-05-01T22:55:00Z

# Working Directory Checkpoints and Rollback Module Inventory

## Existing ownership boundaries

- `src/worktree/` owns session-level git worktree isolation. It already discovers git repo roots via `WorktreeManager::find_repo_root`, creates per-session worktrees in `create.rs`, records active worktrees through `registry.rs`, and completes/merges isolated session branches through `session_bridge.rs`. This is adjacent to, but distinct from, the requested working-directory checkpoint feature because `useWorktrees` moves the whole session into a hidden worktree, while checkpoints should protect the current working tree before mutating tools run.
- `src/tools/git_ops/` owns low-level git repository operations. Existing helpers cover status, staging, commits, ref checks, and diff rendering. The first checkpoint backend should extend or reuse this boundary for git-native snapshots rather than shelling out from unrelated tools.
- File-mutating tools live under `src/tools/`: `write`, `edit`, `patch`, shell-like mutation via `bash`, and likely generated writes from higher-level tools. The checkpoint trigger should wrap mutating tool execution centrally where possible instead of duplicating snapshot calls inside each tool body.
- Shared tool registration is in `src/modes/common.rs`. A user-visible rollback/checkpoint tool, if exposed to the agent, should register through the existing tiered tool path so prompt mode, TUI, and daemon sessions receive the same surface.
- CLI command definitions live in `src/cli.rs` and command handlers in `src/commands/`. A human-facing rollback command should be added here rather than hidden behind the agent-only tool surface.
- Session persistence lives in `crates/clankers-session` and tool result details already flow through `ToolResult.details` into persisted tool outcomes. Checkpoint creation/rollback metadata should use safe normalized details rather than raw patch contents or secret-bearing file data.
- Configuration lives in `crates/clankers-config/src/settings.rs`. A production slice should add disabled/explicit policy only if checkpointing is optional or tunable; otherwise defaults should be documented and validated at the command/tool boundary.

## Existing gaps

- No current module creates a pre-mutation snapshot of the active working directory before `write`, `edit`, `patch`, or `bash` changes files.
- No rollback CLI/TUI flow exists for restoring a named checkpoint distinct from session history or worktree merge/recovery.
- Worktree isolation is opt-in and not a substitute for checkpoint/rollback in the user's current checkout.
- `git_ops` does not yet expose stash/create-ref/apply-style checkpoint primitives such as durable `refs/clankers/checkpoints/*` refs.

## Recommended first-pass boundary

- Add a `src/checkpoints/` or `src/worktree/checkpoint.rs` functional core for git-backed checkpoint records: discover repo, create a durable checkpoint ref from current index/worktree state, list checkpoints, and apply/restore a checkpoint with explicit errors.
- Keep the backend local and git-only in the first slice. Non-git directories should return actionable unsupported errors.
- Add a human-facing `clankers checkpoint ...` or `clankers rollback ...` command for create/list/restore, plus a small agent-visible tool only if the design requires model-callable rollback.
- Wrap or instrument file-mutating tool dispatch centrally so automatic checkpoint metadata can record `source=working_directory_checkpoint`, action, status, repo root, checkpoint id/ref, changed-file counts, and sanitized errors. Do not persist full diffs or file contents in session metadata.

## Verification implications

- Unit tests should cover non-git rejection, durable checkpoint naming/metadata construction, and command/tool policy validation.
- Integration tests should create a temporary git repo, mutate a file after a checkpoint, restore the checkpoint, and verify the file content returns to the checkpointed state.
- Documentation should clearly distinguish session checkpoints/worktrees from working-directory rollback.
