# Native process tool compatibility inventory

Source: `src/tools/process.rs` as inspected on 2026-05-17.

## Public actions to preserve

The current tool schema exposes these actions: `start`, `list`, `poll`, `log`, `wait`, `kill`, `write`, `submit`, and `close`.

### `start`

Compatibility expectations:

- Accept exactly one of:
  - `command` for shell mode via `bash -c`.
  - `program` plus optional string-array `args` for direct exec mode.
- Reject requests that provide both `command` and `program` with `Provide either 'command' or 'program', not both.`
- Reject requests missing both with `Missing required parameter: command or program`.
- Preserve current bash safety behavior: shell-mode `command` is checked with `crate::tools::bash::check_dangerous(command)` before spawn, and blocked commands return a tool error instead of starting a background process.
- Direct exec mode must not pass through shell interpolation; command preview is formatted with shell-display quoting for args.
- Spawned native processes must receive the sanitized tool environment, piped stdin/stdout/stderr, and on Linux run in a dedicated process group so kill can terminate descendants.
- Returned start text currently includes stable in-memory id and OS pid: `Started background process proc_N (pid: <pid|unknown>)`.
- The procmon hook, when installed, registers PID metadata with tool name `process`, truncated command preview, and tool call id.

### `list`

Compatibility expectations:

- Return `No background processes.` when the registry is empty.
- Sort entries by current process session id.
- Return a table with columns `SESSION`, `STATUS`, `AGE`, and `COMMAND`.
- Include safe/truncated command preview and current status label.

### `poll`

Compatibility expectations:

- Require `session_id`; missing ids return `Missing required parameter: session_id`.
- Unknown ids return `Unknown process session_id: <id>`.
- Return current status plus only output lines not previously returned by `poll`/`wait`; cursor advances after each drain.
- Return `No new output.` if there are no new lines.

### `log`

Compatibility expectations:

- Require `session_id`; unknown ids return the same unknown-session error as poll.
- Return a bounded snapshot of retained output lines.
- Default `limit` is 200 lines.
- Default `offset` is the last `limit` lines.
- Include `start..end of total` and status in the response.
- Return explicit empty-log text when no lines are available.

### `wait`

Compatibility expectations:

- Require `session_id`; unknown ids return the same unknown-session error.
- Default timeout is 30 seconds.
- Timeout `0` means keep waiting until terminal status under current implementation.
- While running, sleep/poll in small intervals and return `<id> still running after <timeout>s` on timeout.
- On terminal status, drain newly available output and include it after the terminal status line.

### `kill`

Compatibility expectations:

- Require `session_id`; unknown ids return the same unknown-session error.
- If already terminal, return `<id> is already <status>`.
- First kill request sends the stored kill channel and returns `Kill requested for <id>`.
- Repeated kill requests after channel consumption return `Kill already requested for <id>`.
- Native Unix kill targets the process group with SIGTERM, waits briefly, then SIGKILLs if needed; non-Unix/fallback uses `start_kill()`.

### `write` and `submit`

Compatibility expectations:

- Require `session_id`; unknown ids return the same unknown-session error.
- If not running, return `<id> is not running (<status>)`.
- If stdin is not available/open, return `<id> has no open stdin`.
- `write` sends `data` exactly as bytes.
- `submit` sends `data` plus a trailing newline.
- Both flush stdin and report bytes written; newline counts as one byte.

### `close`

Compatibility expectations:

- Require `session_id`; unknown ids return the same unknown-session error.
- Drop stored stdin once and return `Closed stdin for <id>`.
- Repeated close returns `Stdin already closed for <id>`.

## Status vocabulary mapping

Current native status labels map to durable DTOs as follows:

- `running` → `ProcessJobStatus::Running`.
- `exited(<code>)@<elapsed>` → `ProcessJobStatus::Succeeded { exit_code: Some(code) }` when code is zero; otherwise `ProcessJobStatus::Failed { exit_code: Some(code), reason }`.
- `exited(signal)@<elapsed>` → `ProcessJobStatus::Failed { exit_code: None, reason: "signal" }`.
- `killed@<elapsed>` → `ProcessJobStatus::Killed`.
- `failed@<elapsed>(<message>)` → `ProcessJobStatus::Failed { exit_code: None, reason: message }`.

## Durable-backend constraints

- Native remains the default backend when no backend override is supplied.
- Durable backends must preserve the above parser/error behavior at the tool boundary, even when backend-specific actions are unsupported.
- Pueue/systemd backends may return `unsupported_action_for_backend` for stdin/direct process-group semantics they cannot support, but must not silently no-op.
- All backend receipts must include the stable Clankers id, backend kind, typed status, and safe backend reference when available.
- Existing dangerous shell command policy stays before backend dispatch; a blocked shell command must not create native, pueue, or systemd work.
