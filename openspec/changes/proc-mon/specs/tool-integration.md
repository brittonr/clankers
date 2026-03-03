# Tool Integration — PID Registration Points

## Overview

Each tool that spawns child processes needs a one-line integration to
register the PID with the `ProcessMonitor`. This is purely additive —
no existing behavior changes.

## Integration Points

### BashTool (`src/tools/bash.rs`)

After `cmd.spawn()` succeeds, register the child PID:

```rust
let mut child = match cmd.spawn() {
    Ok(c) => c,
    Err(e) => return ToolResult::error(format!("Failed to spawn bash: {}", e)),
};

// NEW: register with process monitor
if let Some(ref monitor) = self.process_monitor {
    monitor.register(child.id().unwrap_or(0), ProcessMeta {
        tool: "bash".into(),
        command: command.to_string(),
        call_id: ctx.call_id.clone(),
        display_name: truncate_command(command, 40),
    });
}
```

The `BashTool` struct gains an optional `Arc<ProcessMonitor>` field,
injected at construction. If `None` (tests, headless without monitoring),
registration is skipped.

### SubagentTool (`src/tools/subagent.rs`)

After spawning the clankers subprocess:

```rust
// After tokio::process::Command::new("clankers").spawn()
if let Some(ref monitor) = self.process_monitor {
    monitor.register(child.id().unwrap_or(0), ProcessMeta {
        tool: "subagent".into(),
        command: task_description.to_string(),
        call_id: ctx.call_id.clone(),
        display_name: format!("[subagent] {}", truncate_command(&task_description, 30)),
    });
}
```

### DelegateTool (`src/tools/delegate.rs`)

Same pattern for local worker subprocess spawning:

```rust
if let Some(ref monitor) = self.process_monitor {
    monitor.register(child.id().unwrap_or(0), ProcessMeta {
        tool: "delegate".into(),
        command: format!("worker:{} {}", worker_name, task),
        call_id: ctx.call_id.clone(),
        display_name: format!("[worker:{}]", worker_name),
    });
}
```

Remote delegations (iroh RPC) are NOT registered — they run on a
different machine and we can't read their `/proc`.

## ProcessMonitor Handle

Tools receive the monitor via a shared handle type:

```rust
type ProcessMonitorHandle = Option<Arc<ProcessMonitor>>;
```

This is threaded through tool construction, similar to how `panel_tx`
is passed to subagent/delegate tools today:

```rust
impl BashTool {
    pub fn with_process_monitor(mut self, monitor: Arc<ProcessMonitor>) -> Self {
        self.process_monitor = Some(monitor);
        self
    }
}
```

## Helper Functions

```rust
/// Truncate a command string for display, preserving the binary name.
fn truncate_command(cmd: &str, max_len: usize) -> String {
    if cmd.len() <= max_len {
        cmd.to_string()
    } else {
        format!("{}…", &cmd[..max_len - 1])
    }
}
```

## Unregistration

Tools do NOT need to unregister PIDs. The `ProcessMonitor` automatically
detects when a PID exits during its poll loop and moves it to history.
This avoids race conditions where a tool might try to unregister before
the monitor has sampled the final state.
