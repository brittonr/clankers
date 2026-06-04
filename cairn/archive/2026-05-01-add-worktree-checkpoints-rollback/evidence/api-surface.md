Artifact-Type: api-surface
Task-ID: define-user-facing-surface
Covers: r[checkpoints-rollback.capability], r[checkpoints-rollback.scenario.unsupported-config], r[checkpoints-rollback.observability]
Generated: 2026-05-01T22:31:40Z

# Working Directory Checkpoints and Rollback API Surface

## First-pass user-facing surface

### CLI

Add a top-level command group:

```text
clankers checkpoint create [--label <LABEL>] [--cwd <DIR>] [--json]
clankers checkpoint list [--cwd <DIR>] [--json]
clankers rollback <CHECKPOINT_ID> [--cwd <DIR>] [--yes] [--json]
```

Semantics:

- `checkpoint create` snapshots the current git working directory and prints a stable checkpoint id/ref, changed-file counts, and repo root.
- `checkpoint list` lists locally-created clankers checkpoint records for the current repository.
- `rollback <CHECKPOINT_ID>` restores tracked and untracked working-tree content to the named checkpoint. It must require `--yes` for non-interactive execution when destructive changes are present.
- Global `--cwd` continues to select the repository root/discovery starting point; command-local `--cwd` is not needed because the existing top-level flag already resolves `CommandContext.cwd`.
- `--json` should produce a structured output shape suitable for daemon/script callers.

### Agent tool

Add a specialty tool named `checkpoint` only after the functional core is in place. Supported actions:

```json
{"action":"create","label":"before-large-refactor"}
{"action":"list"}
{"action":"rollback","checkpoint_id":"clankers-checkpoint-...","confirm":true}
```

Tool results must include safe metadata in `details`: `action`, `status`, `repo_root`, `checkpoint_id`, `backend`, `changed_file_count`, and sanitized `error_code`/`error_message` when failing. They must not include raw diffs, file contents, secret values, or full environment variables.

### TUI / slash command

Expose `/checkpoint` and `/rollback <id>` as thin wrappers around the same command/service boundary only if slash commands already have access to the same confirmation path. Otherwise first-pass TUI support is documented as "use the CLI command from a shell" while prompt/daemon paths can use the tool.

### Config

No required config key in the first pass. The supported backend is local git in the active repository. Future policy may add settings such as `checkpoint.autoBeforeMutatingTools`, retention, or backend choice, but this slice should avoid a partially-supported config surface.

## Unsupported first-pass cases

- Non-git directories: return an actionable unsupported error.
- Remote/shared checkpoint backends: unsupported.
- Submodule recursion and nested repository restoration: unsupported unless the target checkpoint was created at that exact nested repo root.
- Binary safety review and semantic merge rollback: checkpoint restore is file-level, not a semantic conflict resolver.
- Automatic checkpoint before every tool mutation: may be added after the explicit CLI/tool path is proven. If implemented in this slice, it must be opt-in and bounded to local git repos.
- Rollback without explicit confirmation when current working-tree changes would be overwritten.

## Policy boundaries

- The backend is local-only and git-backed.
- Error messages should name the unsupported condition and the next action, e.g. "not a git repository; run from a git checkout or pass --cwd".
- No provider credentials or API configuration participate in checkpoint operations.
- Session/replay details record identifiers and counts, not content.
- Rollback should refuse if the checkpoint id is not under the clankers checkpoint namespace.

## Implementation target

Use a small service layer (`src/checkpoints/` or `src/worktree/checkpoint.rs`) with a stable output struct consumed by CLI, tool, and tests. Keep git operations in or near `src/tools/git_ops/` so the implementation remains testable without TUI or provider setup.
