## Context

Clankers has a tiered tool system (`Core`, `Orchestration`, `Specialty`, `Matrix`) registered in `src/modes/common.rs`. Tools implement the `Tool` trait from `crates/clankers-agent/src/tool.rs`, receiving a `ToolContext` with cancellation, event bus, hooks, and DB access. The TUI is ratatui-based with a widget host system and plugin UI support (`crates/clankers-tui-types/src/plugin.rs`). Git operations are done via shell commands. Session state is persisted as JSONL via `crates/clankers-session/`.

The autoresearch pattern (from karpathy/autoresearch) requires: experiment initialization, timed command execution with output capture, structured metric extraction, result logging with auto-commit/revert, confidence scoring, and a live dashboard. The pi-autoresearch skill already defines the UX contract — clankers needs to provide the underlying tools natively so they work in daemon mode, headless mode, and without external MCP dependencies.

## Goals / Non-Goals

**Goals:**
- Provide `init_experiment`, `run_experiment`, `log_experiment` as native clankers agent tools.
- JSONL experiment log for persistence and resume across context resets.
- Git automation: branch creation, auto-commit on `keep`, auto-revert on `discard`/`crash`/`checks_failed` (preserving autoresearch files from revert).
- Metric extraction from `METRIC name=value` stdout lines.
- Confidence scoring after 3+ datapoints (improvement as multiple of noise floor).
- TUI dashboard: experiment table, best result, progress indicator.
- Work in interactive TUI, headless, and daemon modes.

**Non-Goals:**
- The agent loop logic itself — that stays in skill files / user prompts, not in code.
- ML-specific features (GPU monitoring, VRAM tracking) — those are domain-specific to the user's `autoresearch.sh`.
- Multi-GPU or distributed experiment coordination.
- Plugin (WASM) implementation — the tools need shell execution, git, and filesystem access that WASM sandboxing prohibits. A future WASM dashboard-only plugin could complement this, but the tools must be native.

## Decisions

### D1: New crate `crates/clankers-autoresearch/` rather than spreading across existing crates

The experiment lifecycle (init, run, log, metrics, confidence, JSONL) is a cohesive domain. A dedicated crate keeps it testable in isolation and avoids polluting `clankers-agent` or the main binary with experiment semantics.

**Alternative**: Implement directly in `src/tools/`. Rejected because the state management (ExperimentSession, metric history, confidence math) is substantial enough to warrant its own crate with unit tests.

### D2: `Specialty` tool tier, not a new tier

Autoresearch tools are opt-in interactive tools, same category as `todo`, `web`, `commit`. Adding a new tier (e.g. `Research`) would require changes to tier activation logic and CLI flags for minimal benefit.

**Alternative**: New `ToolTier::Research`. Rejected — not enough tools to justify a tier; `Specialty` already has the right activation semantics.

### D3: JSONL log format matching pi-autoresearch conventions

The JSONL file (`autoresearch.jsonl`) uses the same schema as the existing pi-autoresearch skill: a config header line followed by one JSON object per experiment. This lets existing analysis tools (notebooks, dashboards) work unchanged and makes migration seamless.

Schema per line:
```json
{"type": "config", "name": "...", "metric_name": "...", "metric_unit": "...", "direction": "..."}
{"type": "result", "run": 1, "commit": "abc1234", "metric": 0.997, "metrics": {...}, "status": "keep", "description": "...", "asi": {...}, "timestamp": "..."}
```

### D4: Git operations via `Command` (shell git), not `git2`

The codebase already uses shell git everywhere. Consistency matters more than the marginal API benefits of libgit2. The operations are simple: branch, add, commit, reset, diff.

### D5: Confidence scoring via rolling standard deviation

After the baseline + 2 more runs (3 total), compute the noise floor as the standard deviation of the last N kept results' metrics. Report each new result's improvement as `delta / noise_floor`. Values ≥ 2.0× indicate likely-real improvements. This is advisory — never auto-discard.

### D6: Revert preserves autoresearch files

On `discard`/`crash`/`checks_failed`, the tool runs `git checkout -- .` but first stashes `autoresearch.jsonl`, `autoresearch.md`, `autoresearch.sh`, `autoresearch.checks.sh`, `autoresearch.ideas.md`, and `autoresearch.config.json` so they survive the revert. This matches the pi-autoresearch convention.

### D7: TUI dashboard as a built-in widget, not a plugin widget

The dashboard reads `autoresearch.jsonl` directly and renders via ratatui. Plugin widgets go through the `PluginUiState` indirection which adds complexity for no sandboxing benefit here. The widget is toggled via a keybinding (Ctrl+X, matching pi convention) or leader menu entry.

### D8: `run_experiment` supports optional checks script

If `autoresearch.checks.sh` exists, `run_experiment` executes it after a passing benchmark. Checks timeout is configurable (default 300s). Checks failure is reported distinctly so the agent can log as `checks_failed`. Checks execution time is excluded from the primary metric timing.

## Risks / Trade-offs

- **[Git state corruption]** → Auto-revert uses `git checkout -- .` which is safe for tracked files. Untracked experiment files are explicitly preserved. The tool refuses to operate on a dirty worktree at init time.
- **[JSONL append-only growth]** → For typical overnight runs (~100 experiments), this is <100KB. Not a concern.
- **[Confidence scoring on non-stationary metrics]** → Rolling window (last 10 kept results) limits the impact of distribution shift. The score is advisory only.
- **[No WASM plugin path]** → Tools need shell exec + git + filesystem. A future "dashboard-only" WASM plugin could read the JSONL and render widgets, but the tools themselves must be native. This is the right tradeoff for now.
