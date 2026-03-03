# proc-mon — Design

## Decisions

### Poll `/proc` directly, don't shell out to `ps`

**Choice:** Read `/proc/<pid>/stat`, `/proc/<pid>/status`, `/proc/<pid>/statm` directly.
**Rationale:** Zero subprocess overhead. Already in-process. Avoids parsing
ps's text output. Works in sandboxed environments where spawning extra
processes is undesirable. On macOS, fall back to `libproc` / `sysctl` via
the `sysinfo` crate.
**Alternatives considered:** Shelling out to `ps aux`, using `top -b -n1`.
Both spawn processes to monitor processes — circular. The `sysinfo` crate
wraps the platform-specific APIs cleanly.

### Use the `sysinfo` crate for cross-platform abstraction

**Choice:** `sysinfo` crate for CPU/memory/PID queries.
**Rationale:** Mature, well-maintained, cross-platform (Linux, macOS, Windows).
Handles the `/proc` parsing, clock tick conversion, and platform differences.
Already used widely in the Rust ecosystem. Avoids hand-rolling `/proc` parsers.
**Alternatives considered:** `procfs` (Linux-only), `psutil-rs` (unmaintained),
raw `/proc` reads (non-portable). `sysinfo` is the pragmatic choice.

### Register PIDs at spawn site, don't scan all system processes

**Choice:** Explicit PID registration from bash/subagent/delegate tools.
**Rationale:** Clankers only cares about its own children, not everything on
the system. Registration is O(1) vs. full system scan. Also captures the
semantic context (which tool spawned it, what command, what call_id).
**Alternatives considered:** Full process table scan filtered by ppid. Works
but wastes cycles and loses the "why was this spawned" metadata.

### 2-second default polling interval

**Choice:** Sample resource usage every 2 seconds.
**Rationale:** Fast enough to catch spikes, slow enough to be negligible overhead.
`htop` defaults to 1.5s. We're slightly slower because we also walk child trees.
Configurable via `ProcessMonitorConfig`.
**Alternatives considered:** 500ms (too chatty for event bus), 5s (misses short
spikes), event-driven via `waitid(WNOHANG)` (only catches exits, not resource
changes).

### Flat process table in TUI, not tree view

**Choice:** Flat sorted table with an indentation column for depth.
**Rationale:** Process trees are usually 2-3 levels deep (clankers → bash →
cargo → rustc). A full tree widget adds complexity for minimal benefit. Flat
table with indent + parent column gives the same info. Sortable by CPU/MEM/time.
**Alternatives considered:** `tree_view.rs` integration. More visual but the
existing tree view is for file hierarchies, not process lists. Would need a
new tree data adapter.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     Agent Loop                          │
│                                                         │
│  ┌──────────┐  ┌──────────────┐  ┌────────────────┐    │
│  │ BashTool │  │ SubagentTool │  │ DelegateTool   │    │
│  └─────┬────┘  └──────┬───────┘  └───────┬────────┘    │
│        │               │                  │             │
│        └───────────────┼──────────────────┘             │
│                        │ register_process(pid, meta)    │
│                        ▼                                │
│  ┌─────────────────────────────────────────────────┐    │
│  │              ProcessMonitor                      │    │
│  │                                                  │    │
│  │  tracked: HashMap<u32, TrackedProcess>           │    │
│  │  system:  sysinfo::System (refreshed on poll)    │    │
│  │  config:  ProcessMonitorConfig                   │    │
│  │                                                  │    │
│  │  ┌─────────────────────────────────┐             │    │
│  │  │  Poll Loop (every 2s)           │             │    │
│  │  │  1. Refresh PIDs in sysinfo     │             │    │
│  │  │  2. Sample CPU/RSS per PID      │             │    │
│  │  │  3. Walk child tree (ppid)      │             │    │
│  │  │  4. Detect exits (reap)         │             │    │
│  │  │  5. Emit AgentEvent::Process*   │             │    │
│  │  └─────────────────────────────────┘             │    │
│  └──────────────────┬──────────────────────────────┘    │
│                     │                                   │
│        ┌────────────┼────────────┐                      │
│        ▼            ▼            ▼                      │
│  ┌──────────┐ ┌──────────┐ ┌──────────────┐            │
│  │ EventBus │ │ ProcMon  │ │ ProcessPanel │            │
│  │ (events) │ │  Tool    │ │   (TUI)      │            │
│  └──────────┘ └──────────┘ └──────────────┘            │
└─────────────────────────────────────────────────────────┘
```

## Data Flow

### Process registration
1. BashTool spawns `bash -c "cargo build"`, gets PID 12345
2. BashTool calls `monitor.register(12345, ProcessMeta { tool: "bash", command: "cargo build", call_id: "xyz" })`
3. Monitor starts tracking PID 12345 on next poll cycle

### Resource sampling
1. Poll loop fires every 2s
2. For each tracked PID: refresh via `sysinfo`, read CPU %, RSS, state
3. Walk `/proc/<pid>/task/` or `children` for subprocess tree
4. Emit `AgentEvent::ProcessSample { pid, cpu, rss, children }` on event bus
5. TUI panel receives event, updates its table

### Process exit
1. Poll detects PID no longer exists (or `sysinfo` reports zombie/dead)
2. Record final stats: wall time, peak RSS, total CPU time, exit code
3. Emit `AgentEvent::ProcessExit { pid, stats }`
4. Move from `active` to `history` in the monitor
5. TUI shows process as completed with final stats

### Agent self-inspection
1. Agent calls `procmon` tool with action `summary` or `list`
2. Tool reads from `ProcessMonitor` shared state
3. Returns formatted table of active processes + aggregate stats
