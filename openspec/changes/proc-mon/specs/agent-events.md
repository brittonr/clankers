# Agent Events — Process Lifecycle

## Overview

New `AgentEvent` variants for process lifecycle, emitted by the
`ProcessMonitor` on the existing event bus. The TUI and JSON mode
consumers can react to these for live display and logging.

## New Event Variants

Add to `src/agent/events.rs`:

```rust
enum AgentEvent {
    // ... existing variants ...

    // Process monitoring
    ProcessSpawn {
        pid: u32,
        tool: String,
        command: String,
        call_id: String,
    },
    ProcessSample {
        /// Batch of all active process snapshots (one event per poll cycle)
        processes: Vec<ProcessSnapshot>,
        aggregate: AggregateSnapshot,
    },
    ProcessExit {
        pid: u32,
        exit_code: Option<i32>,
        wall_time: Duration,
        peak_rss_bytes: u64,
        total_cpu_seconds: f64,
    },
}
```

### Supporting Types

```rust
/// Lightweight snapshot for event transmission (no Arc, no locks)
struct ProcessSnapshot {
    pid: u32,
    tool: String,
    display_name: String,
    cpu_percent: f32,
    rss_bytes: u64,
    wall_time: Duration,
    child_count: usize,
    depth: u8,  // 0 = direct child, 1+ = grandchild
}

struct AggregateSnapshot {
    active_count: usize,
    total_rss_bytes: u64,
    total_cpu_percent: f32,
}
```

## Event Flow

### ProcessSpawn
- **Emitted by:** `ProcessMonitor::register()` (called from bash/subagent/delegate tools)
- **When:** Immediately when a PID is registered
- **Consumed by:** TUI process panel (adds row), JSON mode (logs spawn)

### ProcessSample
- **Emitted by:** `ProcessMonitor` poll loop
- **When:** Every poll interval (2s), only if there are active processes
- **Consumed by:** TUI process panel (updates table), status bar (aggregate display)
- **Note:** Batched — one event contains ALL active process snapshots. This
  avoids flooding the event bus with per-process events at high process counts.

### ProcessExit
- **Emitted by:** `ProcessMonitor` poll loop (during reap phase)
- **When:** A tracked PID is detected as exited
- **Consumed by:** TUI process panel (marks row as done), JSON mode (logs completion)

## Event Bus Integration

The `ProcessMonitor` takes an `Option<broadcast::Sender<AgentEvent>>` at
construction, same pattern as `ToolContext`. If no sender is provided
(headless/test), events are silently dropped.

The monitor MUST NOT block on send. Use `try_send` or ignore errors —
the event bus is best-effort for monitoring data.
