Artifact-Type: session-metadata-evidence
Task-ID: persist-log-session-metadata
Covers: r[checkpoints-rollback.observability], r[checkpoints-rollback.scenario.session-metadata]
Generated: 2026-05-01T22:54:00Z

# Checkpoint Session Metadata Evidence

## Persistence path

The `checkpoint` tool returns `ToolResult::with_details(...)` for both successful and failed checkpoint operations in `src/tools/checkpoint.rs`. Tool result details are part of the canonical `ToolResult` shape in `crates/clanker-message/src/tool_result.rs` and flow through normal session/tool-result recording.

## Metadata shape

Checkpoint operations record normalized metadata from `CheckpointMetadata` in `src/checkpoints.rs`:

- `action`
- `status`
- `backend`
- `repo_root`
- `checkpoint_id`
- `changed_file_count`
- `error_code`
- `error_message`

## Redaction / replay safety

The details deliberately omit raw diffs, file contents, environment variables, and provider credentials. Error messages are flattened and length-bounded by `sanitize_error_message` before being placed in details.

## Verification

`CARGO_TARGET_DIR=target cargo nextest run -p clankers checkpoint --no-fail-fast` passed with checkpoint metadata tests covering safe detail shape, namespace policy, sanitized errors, successful create/list/rollback, non-git failure, CLI parsing, and shared tool publication.
