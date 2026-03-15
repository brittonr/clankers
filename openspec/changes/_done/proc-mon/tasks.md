# proc-mon — Tasks

## Phase 1: Core monitor (no TUI, no tool) ✅

- [x] Add `sysinfo` crate to `Cargo.toml` dependencies
- [x] Create `src/procmon/mod.rs` with `ProcessMonitor` struct and config
- [x] Implement `TrackedProcess`, `ProcessMeta`, `ResourceSnapshot`, `ProcessState` types
- [x] Implement `ProcessMonitor::register(pid, meta)` — adds PID to tracked set
- [x] Implement poll loop — refresh sysinfo, sample CPU/RSS per tracked PID
- [x] Implement child tree walking — discover grandchild PIDs via ppid
- [x] Implement exit detection — move exited PIDs to history ring buffer
- [x] Implement `AggregateStats` computation (total RSS, CPU, counts)
- [x] Add `ProcessSpawn`, `ProcessSample`, `ProcessExit` variants to `AgentEvent`
- [x] Emit events on the agent event bus from the poll loop
- [x] Wire `ProcessMonitor` startup/shutdown into the agent session lifecycle
- [x] Unit tests: registration, sampling mock, exit detection, history eviction

## Phase 2: Tool integration (register PIDs from existing tools) ✅

- [x] Add `ProcessMonitorHandle` field to `BashTool`, with `with_process_monitor()` builder
- [x] Register spawned PID in `BashTool::execute()` after `cmd.spawn()`
- [x] Add `ProcessMonitorHandle` field to `SubagentTool`, register spawned subagent PIDs
- [x] Add `ProcessMonitorHandle` field to `DelegateTool`, register local worker PIDs
- [x] Thread `Arc<ProcessMonitor>` through tool construction in `main.rs`
- [x] Integration test: spawn bash command, verify monitor tracks it, verify exit event

## Phase 3: procmon tool (agent self-inspection) ✅

- [x] Create `src/tools/procmon.rs` implementing `Tool` trait
- [x] Implement `list` action — formatted active process table
- [x] Implement `summary` action — aggregate stats one-liner
- [x] Implement `inspect` action — deep dive on single PID with children
- [x] Implement `history` action — recently completed processes
- [x] Register `procmon` tool in `src/tools/mod.rs`
- [x] Add procmon to the system prompt's tool descriptions
- [x] Unit tests: each action with mock monitor state

## Phase 4: TUI process panel ✅

- [x] Create `src/tui/components/process_panel.rs` implementing `Panel` trait
- [x] Implement list view — active process table with columns (PID, CPU, MEM, TIME, CMD)
- [x] Implement child indentation (depth-based `└─` prefix)
- [x] Implement sort cycling (CPU → Memory → Time → Name)
- [x] Implement completed section below separator
- [x] Implement color coding (CPU/RSS thresholds)
- [x] Implement keybindings (sort, toggle completed, navigate, detail view)
- [x] Implement detail view — single process deep dive with children list
- [x] Implement sparkline history (Unicode block chars for last N samples)
- [x] Register panel in `src/tui/components/mod.rs` and panel registry
- [x] Add aggregate stats segment to status bar (`src/tui/components/status_bar.rs`)
- [x] Wire `AgentEvent::ProcessSample` / `ProcessExit` into panel event handler
