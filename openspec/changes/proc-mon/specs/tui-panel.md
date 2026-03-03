# Process Panel — TUI Display

## Overview

A new TUI panel (`process_panel.rs`) that shows a live-updating process
table. Follows the same `Panel` trait pattern as `subagent_panel.rs` and
`environment_panel.rs`.

## Layout

```
┌─ Processes (3 active · 1.7 GB RSS · 68% CPU) ─────────────────┐
│                                                                 │
│  PID    CPU%   MEM      TIME     COMMAND                        │
│  12345  45.2%  1.2 GB   02:34    cargo build --release          │
│  12389  12.1%  256 MB   00:45     └─ rustc clankers             │
│  12401   8.3%  128 MB   00:12     └─ rustc clankers_router      │
│  23456   0.1%  48 MB    05:12    [subagent] review auth         │
│  23478   2.4%  96 MB    00:03     └─ rg "auth" --json           │
│                                                                 │
│  ── completed ──────────────────────────────────────────────    │
│  11234  ✓      2.1 GB   03:45    cargo test                     │
│  11100  ✗(1)   512 MB   00:02    rg "nonexistent"               │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Panel Behavior

### Title Bar
Dynamic title showing: active count, total RSS, total CPU.
Updates every poll cycle.

### Active Section
- Sorted by CPU% descending (default), toggleable via keybindings
- Child processes indented with `└─` prefix
- Tool type shown in brackets for non-bash (`[subagent]`, `[delegate]`)
- Command truncated to fit column width, full command in tooltip/detail view
- Color coding:
  - CPU > 80%: red
  - CPU > 50%: yellow
  - RSS > 1 GB: yellow
  - RSS > 4 GB: red
  - Otherwise: default text color

### Completed Section
- Shown below a separator line
- Limited to last 5 completed (configurable)
- Exit code shown as `✓` (0) or `✗(N)` (non-zero)
- Peak RSS and wall time shown
- Dimmed text style to de-emphasize

### Empty State
When no processes are tracked:
```
  No processes running
```

## Keybindings

| Key | Action |
|-----|--------|
| `s` | Cycle sort: CPU → Memory → Time → Name |
| `c` | Toggle completed section visibility |
| `Enter` | Open detail view for selected process |
| `Esc` | Close detail view, return to list |
| `↑`/`↓` | Navigate process list |
| `k`/`j` | Vim-style navigation |

## Detail View

Pressing Enter on a process opens a detail view (same pattern as
subagent panel's detail mode):

```
┌─ Process 12345 — cargo build --release ────────────────────────┐
│                                                                 │
│  Tool:       bash                                               │
│  Call ID:    toolu_abc123                                        │
│  State:      running                                            │
│  CPU:        45.2% (peak: 98.7%)                                │
│  RSS:        1.2 GB (peak: 2.1 GB)                              │
│  VMS:        4.8 GB                                             │
│  Threads:    12                                                 │
│  Wall time:  02:34                                              │
│                                                                 │
│  Children:                                                      │
│    12389  rustc --crate-name clankers       12.1%  256 MB       │
│    12401  rustc --crate-name clankers_rtr    8.3%  128 MB       │
│                                                                 │
│  ── Resource History (last 30 samples) ────────────────────    │
│  CPU:  ▁▂▃▅▇█████▇▇▅▅▃▃▃▅▅▇█▇▅▃▃▂▂▁▁                         │
│  RSS:  ▁▁▂▃▃▅▅▅▆▆▇▇▇▇████████████████                         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

The sparkline-style history shows the last N samples as a mini chart
using Unicode block characters (`▁▂▃▄▅▆▇█`). This gives a quick visual
of whether resource usage is climbing, stable, or dropping.

## Implementation

File: `src/tui/components/process_panel.rs`

### State

```rust
struct ProcessPanel {
    /// Live process data, updated from AgentEvent::ProcessSample
    active: Vec<ProcessRow>,
    /// Recently completed, from AgentEvent::ProcessExit
    completed: VecDeque<CompletedRow>,
    /// Current sort column
    sort_by: SortColumn,
    /// Selected row index (for navigation)
    selected: usize,
    /// Detail view PID (None = list view)
    detail_pid: Option<u32>,
    /// Show completed section
    show_completed: bool,
    /// Resource history for sparklines (pid → samples)
    history: HashMap<u32, VecDeque<ResourceSnapshot>>,
}

enum SortColumn { Cpu, Memory, Time, Name }
```

### Event Handling

The panel subscribes to the agent event bus. On each
`AgentEvent::ProcessSample`, it replaces `active` with the new data.
On `AgentEvent::ProcessExit`, it moves the entry to `completed`.

No polling from the TUI side — purely event-driven updates.

## Panel Registration

Add to `src/tui/components/mod.rs` and wire into the panel registry
alongside the existing panels (environment, subagent, session, etc.).
Accessible via the panel switcher keybinding.
