# proc-mon — Tasks

## Phase 1: Core monitor (no TUI, no tool)

- [ ] Add `sysinfo` crate to `Cargo.toml` dependencies
- [ ] Create `src/procmon/mod.rs` with `ProcessMonitor` struct and config
- [ ] Implement `TrackedProcess`, `ProcessMeta`, `ResourceSnapshot`, `ProcessState` types
- [ ] Implement `ProcessMonitor::register(pid, meta)` — adds PID to tracked set
- [ ] Implement poll loop — refresh sysinfo, sample CPU/RSS per tracked PID
- [ ] Implement child tree walking — discover grandchild PIDs via ppid
- [ ] Implement exit detection — move exited PIDs to history ring buffer
- [ ] Implement `AggregateStats` computation (total RSS, CPU, counts)
- [ ] Add `ProcessSpawn`, `ProcessSample`, `ProcessExit` variants to `AgentEvent`
- [ ] Emit events on the agent event bus from the poll loop
- [ ] Wire `ProcessMonitor` startup/shutdown into the agent session lifecycle
- [ ] Unit tests: registration, sampling mock, exit detection, history eviction

## Phase 2: Tool integration (register PIDs from existing tools)

- [ ] Add `ProcessMonitorHandle` field to `BashTool`, with `with_process_monitor()` builder
- [ ] Register spawned PID in `BashTool::execute()` after `cmd.spawn()`
- [ ] Add `ProcessMonitorHandle` field to `SubagentTool`, register spawned subagent PIDs
- [ ] Add `ProcessMonitorHandle` field to `DelegateTool`, register local worker PIDs
- [ ] Thread `Arc<ProcessMonitor>` through tool construction in `main.rs`
- [ ] Integration test: spawn bash command, verify monitor tracks it, verify exit event

## Phase 3: procmon tool (agent self-inspection)

- [ ] Create `src/tools/procmon.rs` implementing `Tool` trait
- [ ] Implement `list` action — formatted active process table
- [ ] Implement `summary` action — aggregate stats one-liner
- [ ] Implement `inspect` action — deep dive on single PID with children
- [ ] Implement `history` action — recently completed processes
- [ ] Register `procmon` tool in `src/tools/mod.rs`
- [ ] Add procmon to the system prompt's tool descriptions
- [ ] Unit tests: each action with mock monitor state

## Phase 4: TUI process panel

- [ ] Create `src/tui/components/process_panel.rs` implementing `Panel` trait
- [ ] Implement list view — active process table with columns (PID, CPU, MEM, TIME, CMD)
- [ ] Implement child indentation (depth-based `└─` prefix)
- [ ] Implement sort cycling (CPU → Memory → Time → Name)
- [ ] Implement completed section below separator
- [ ] Implement color coding (CPU/RSS thresholds)
- [ ] Implement keybindings (sort, toggle completed, navigate, detail view)
- [ ] Implement detail view — single process deep dive with children list
- [ ] Implement sparkline history (Unicode block chars for last N samples)
- [ ] Register panel in `src/tui/components/mod.rs` and panel registry
- [ ] Add aggregate stats segment to status bar (`src/tui/components/status_bar.rs`)
- [ ] Wire `AgentEvent::ProcessSample` / `ProcessExit` into panel event handler
