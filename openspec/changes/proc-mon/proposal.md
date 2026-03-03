# proc-mon — Process Monitoring

## Intent

Clankers spawns child processes constantly — bash commands, subagents, nix
builds, cargo invocations — but has zero visibility into what's actually
running, how much CPU/memory those processes consume, or whether a runaway
`cargo build` is eating all available RAM. You can't tell if the agent is
idle, waiting on a slow command, or thrashing.

This change adds process monitoring so you can see:
- What processes clankers has spawned and their current state
- CPU and memory usage per process (and aggregate)
- Process tree hierarchy (parent → children → grandchildren)
- Historical resource usage for completed processes
- Live resource stats in the TUI

## Scope

### In Scope

- Process tracker that records every child process clankers spawns
- Per-process resource sampling (CPU %, RSS memory, wall time, exit code)
- Process tree tracking (child PIDs spawned by bash, subagent forks, etc.)
- New TUI panel showing live process table with resource usage
- New `procmon` tool the agent can call to inspect its own resource usage
- AgentEvent variants for process lifecycle (spawn, sample, exit)
- Historical stats persisted for the session (peak memory, total CPU time)
- Aggregated resource summary (total RSS, process count, active/finished)

### Out of Scope

- cgroups / resource limits / OOM killing (future: resource-gov change)
- Network I/O tracking per process
- Disk I/O tracking per process
- GPU monitoring
- Remote process monitoring (processes on other machines)
- Profiling or flamegraph integration
- Modifying how bash/subagent tools spawn processes (only observing)

## Approach

A background `ProcessMonitor` task polls `/proc` (Linux) or `sysctl`
(macOS) at a configurable interval (default 2s). Every process spawned
through the bash tool, subagent tool, or delegate tool registers its PID
with the monitor. The monitor samples resource usage, detects child
processes via ppid walking, and emits events on the agent event bus.

A new TUI panel (`process_panel`) renders the live process table. A new
tool (`procmon`) lets the agent query its own resource footprint — useful
for self-awareness ("am I running out of memory?") and for reporting to
the user.

No existing tool code changes behavior. The monitor is purely observational
— it reads from `/proc` and never sends signals or modifies processes.
