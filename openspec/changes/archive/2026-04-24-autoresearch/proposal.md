## Why

Karpathy's autoresearch pattern — autonomous experiment loops that modify code, run a benchmark, keep or discard based on a metric, and repeat indefinitely — has proven effective for overnight optimization of ML training, compiler flags, config tuning, and similar tasks. Clankers currently has no built-in support for this workflow. Users rely on external MCP tools (`init_experiment`, `run_experiment`, `log_experiment`) provided by the pi agent infrastructure, which means the experiment state, results dashboard, and loop control live outside clankers' own tool/TUI system and can't integrate with the daemon, session persistence, or plugin architecture.

Adding first-class autoresearch support lets clankers own the full experiment lifecycle: branching, running, timing, metric extraction, commit/revert, result logging, and a live TUI dashboard — all wired through the existing tool tier system and accessible in both interactive and headless modes.

## What Changes

- Three new agent tools: `init_experiment` (configure session), `run_experiment` (execute + time + capture), `log_experiment` (record result, auto-commit on keep, auto-revert on discard/crash).
- JSONL-based experiment log (`autoresearch.jsonl`) with structured metrics, ASI annotations, and confidence scoring.
- TUI dashboard widget showing experiment history, current best, progress chart, and status — toggled via keybinding or leader menu.
- Git integration: auto-branch (`autoresearch/<tag>`), auto-commit on keep, auto-revert on discard/crash (preserving autoresearch files).
- Optional `autoresearch.checks.sh` support — correctness gate that runs after passing benchmarks.
- Metric extraction from structured `METRIC name=value` stdout lines.
- Confidence scoring: after 3+ runs, report improvement as a multiple of the session noise floor.
- Implement as a new `crates/clankers-autoresearch` crate with tools registered in the `Specialty` tier, keeping the core agent loop unaware of experiment semantics.

## Capabilities

### New Capabilities
- `experiment-lifecycle`: Init/run/log experiment tools, JSONL persistence, metric extraction, confidence scoring, and git branch/commit/revert automation.
- `experiment-dashboard`: TUI widget for live experiment status, result history table, and progress visualization.

### Modified Capabilities
<!-- None — this is additive. -->

## Impact

- New crate: `crates/clankers-autoresearch/` (tools, JSONL state, git ops, metric parsing, confidence math).
- `src/modes/common.rs`: register three new tools in the `Specialty` tier.
- `crates/clankers-tui/`: new dashboard widget, leader menu entry, keybinding.
- System prompt: no changes required — the autoresearch loop is driven by skill files / user instructions, not baked into the prompt.
- Dependencies: no new external deps beyond what's already in the workspace (serde, tokio, git2/command-based git).
