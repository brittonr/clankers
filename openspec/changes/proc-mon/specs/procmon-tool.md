# procmon Tool — Agent Self-Inspection

## Overview

A built-in tool that lets the agent query its own process footprint.
Useful for self-awareness ("my cargo build is using 4GB"), reporting to
the user, and detecting runaway processes.

## Tool Definition

```json
{
  "name": "procmon",
  "description": "Inspect processes spawned by this agent. Shows CPU, memory, and status of all child processes.",
  "input_schema": {
    "type": "object",
    "properties": {
      "action": {
        "type": "string",
        "enum": ["list", "summary", "inspect", "history"],
        "description": "list: active processes table. summary: aggregate stats. inspect: detail for one PID. history: recently completed."
      },
      "pid": {
        "type": "number",
        "description": "PID to inspect (only for action=inspect)"
      },
      "sort": {
        "type": "string",
        "enum": ["cpu", "memory", "time", "name"],
        "description": "Sort order for list/history (default: cpu)"
      },
      "limit": {
        "type": "number",
        "description": "Max entries to return (default: 20)"
      }
    },
    "required": ["action"]
  }
}
```

## Actions

### `list` — Active Process Table

Returns a formatted table of all currently running child processes.

```
PID    CPU%   RSS      TIME     TOOL       COMMAND
12345  45.2%  1.2 GB   02:34    bash       cargo build --release
12389  12.1%  256 MB   00:45    bash       └─ rustc --crate-name clankers
12401   8.3%  128 MB   00:12    bash       └─ rustc --crate-name clankers_router
23456   0.1%  48 MB    05:12    subagent   clankers -p "review auth module"
23478   2.4%  96 MB    00:03    subagent   └─ rg "auth" --json

Active: 5 processes | Total RSS: 1.7 GB | Total CPU: 68.1%
```

### `summary` — Aggregate Stats

Returns a compact overview without per-process detail.

```
Process Monitor Summary
───────────────────────
Active processes:  5
Total RSS:         1.7 GB
Total CPU:         68.1%
Lifetime spawned:  47
Completed:         42
Errors (non-zero): 3
Peak RSS (session): 3.2 GB
```

### `inspect` — Single Process Detail

Deep-dive on one PID, including its child tree and peak stats.

```
PID 12345 — cargo build --release
────────────────────────────────────
Tool:        bash
Call ID:     toolu_abc123
State:       running
CPU:         45.2% (peak: 98.7%)
RSS:         1.2 GB (peak: 2.1 GB)
VMS:         4.8 GB
Threads:     12
Wall time:   02:34
Children:    2 (PIDs: 12389, 12401)
```

### `history` — Recently Completed

Shows processes that have already exited, sorted by most recent.

```
PID    EXIT  PEAK RSS   WALL TIME  COMMAND
11234  0     2.1 GB     03:45      cargo test
11100  1     512 MB     00:02      rg "nonexistent" (error)
10998  0     64 MB      00:01      ls -la
```

## Implementation

File: `src/tools/procmon.rs`

The tool holds an `Arc<RwLock<ProcessMonitorState>>` — the same shared
state the `ProcessMonitor` task writes to. All reads are lock-free on the
happy path (RwLock read guard). No process spawning, no syscalls — just
formatting data that's already been collected.

## Registration

Add to `src/tools/mod.rs`:
```rust
pub mod procmon;
```

Wire into the tool registry the same way other tools are registered,
passing the shared monitor state at construction time.
