# ProcessMonitor — Core Tracking Engine

## Overview

The `ProcessMonitor` is a background task that tracks all child processes
spawned by clankers. It polls resource usage at a fixed interval and
maintains both live and historical state.

## Data Structures

### TrackedProcess

```rust
struct TrackedProcess {
    /// OS process ID
    pid: u32,
    /// Metadata about why/how this was spawned
    meta: ProcessMeta,
    /// Current resource snapshot (updated each poll)
    current: ResourceSnapshot,
    /// Peak values seen across all samples
    peak: ResourcePeak,
    /// Child PIDs discovered via ppid walking
    children: Vec<u32>,
    /// When we started tracking
    registered_at: Instant,
    /// Current state
    state: ProcessState,
}

struct ProcessMeta {
    /// Which tool spawned this ("bash", "subagent", "delegate")
    tool: String,
    /// The command or task description
    command: String,
    /// Tool call ID (links back to agent event stream)
    call_id: String,
    /// Display name (truncated command for TUI)
    display_name: String,
}

struct ResourceSnapshot {
    /// CPU usage as percentage (0.0–100.0 per core)
    cpu_percent: f32,
    /// Resident set size in bytes
    rss_bytes: u64,
    /// Virtual memory size in bytes
    vms_bytes: u64,
    /// Number of threads
    thread_count: u32,
    /// Process state (running, sleeping, zombie, etc.)
    os_state: String,
    /// Timestamp of this sample
    sampled_at: Instant,
}

struct ResourcePeak {
    max_cpu_percent: f32,
    max_rss_bytes: u64,
    max_thread_count: u32,
}

enum ProcessState {
    /// Actively running, being polled
    Active,
    /// Process exited, final stats recorded
    Exited { exit_code: Option<i32>, wall_time: Duration },
    /// Process disappeared without us catching the exit
    Lost,
}
```

### ProcessMonitorConfig

```rust
struct ProcessMonitorConfig {
    /// How often to poll (default: 2s)
    poll_interval: Duration,
    /// How many completed processes to keep in history (default: 100)
    history_limit: usize,
    /// Whether to walk child process trees (default: true)
    track_children: bool,
    /// Whether to emit events on the agent bus (default: true)
    emit_events: bool,
}
```

## Behavior

### Registration

Tools call `monitor.register(pid, meta)` immediately after spawning a
child process. The monitor adds it to the tracked set. If the PID is
already tracked (shouldn't happen, but defensive), the old entry is
replaced.

### Poll Loop

Every `poll_interval`:

1. **Refresh**: Call `sysinfo::System::refresh_processes_specifics()` for
   only tracked PIDs (not the full process table).
2. **Sample**: For each tracked PID, read CPU %, RSS, VMS, thread count,
   state from `sysinfo`. Update `current` snapshot and `peak` watermarks.
3. **Children**: If `track_children` is enabled, walk the process table
   for any PID whose ppid matches a tracked process. Add discovered
   children to `TrackedProcess.children` and start sampling them too.
4. **Reap**: If `sysinfo` reports a PID as gone (not found after refresh),
   transition to `ProcessState::Exited` or `ProcessState::Lost`. Calculate
   wall time. Move from active map to history ring buffer.
5. **Emit**: Send `AgentEvent::ProcessSample` for active processes (batched
   into one event per poll cycle, not per process). Send
   `AgentEvent::ProcessExit` for any newly reaped processes.

### Concurrency

The monitor runs as a single `tokio::spawn` task. Shared state is behind
`Arc<parking_lot::RwLock<ProcessMonitorState>>` so tools can register PIDs
and the procmon tool can read snapshots without blocking the poll loop.

```rust
struct ProcessMonitorState {
    active: HashMap<u32, TrackedProcess>,
    history: VecDeque<TrackedProcess>,  // ring buffer, capped at history_limit
    aggregate: AggregateStats,
}

struct AggregateStats {
    total_active: usize,
    total_rss_bytes: u64,
    total_cpu_percent: f32,
    total_spawned: u64,      // lifetime counter
    total_completed: u64,
    total_errors: u64,       // non-zero exit codes
}
```

### Shutdown

When the agent session ends, the monitor task is cancelled via
`CancellationToken`. Any still-active processes are recorded as-is in
history (they'll be killed by OS when clankers exits anyway).

## File Location

`src/procmon/mod.rs` — new module, not inside `src/tools/`.

The monitor is infrastructure, not a tool. The tool (`src/tools/procmon.rs`)
is a thin wrapper that reads from the monitor's shared state.
