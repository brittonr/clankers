# Status Bar — Aggregate Process Stats

## Overview

Show a compact process summary in the existing TUI status bar, so resource
usage is always visible without switching to the process panel.

## Display

Add a process stats segment to the right side of the status bar:

```
 claude-sonnet-4-5 │ main │ 3 procs · 1.7 GB · 68% CPU │ $0.42
```

### Format

`{count} procs · {total_rss} · {total_cpu}% CPU`

- Only shown when there are active processes (hidden when 0)
- RSS formatted with human-readable units (KB, MB, GB)
- CPU shown as aggregate across all tracked processes

### Color Coding

- Default: dim gray (unobtrusive)
- Total RSS > 4 GB: yellow
- Total RSS > 8 GB: red
- Total CPU > 200%: yellow (multi-core saturation)

## Implementation

The status bar component (`src/tui/components/status_bar.rs`) receives
`AgentEvent::ProcessSample` and caches the latest `AggregateSnapshot`.
On render, it formats the cached data into the status segment.

No new state struct needed — just an `Option<AggregateSnapshot>` field
on the status bar component.
