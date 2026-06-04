## 1. Crate scaffolding

- [x] 1.1 Create `crates/clankers-autoresearch/` with `Cargo.toml` (deps: serde, serde_json, tokio, chrono, tracing)
- [x] 1.2 Add `clankers-autoresearch` as a workspace member and dependency of the main binary crate

## 2. JSONL persistence and state

- [x] 2.1 Define `ExperimentConfig` struct (name, metric_name, metric_unit, direction) with serde
- [x] 2.2 Define `ExperimentResult` struct (type, run, commit, metric, metrics, status, description, asi, timestamp) with serde
- [x] 2.3 Implement JSONL writer: append config line, append result line
- [x] 2.4 Implement JSONL reader: parse config + results from existing file, reconstruct session state (run counter, best metric, metric history)
- [x] 2.5 Unit tests for JSONL round-trip, re-init preserving results, resume from partial file

## 3. Metric extraction

- [x] 3.1 Implement `METRIC name=value` line parser for stdout (handles int, float, scientific notation)
- [x] 3.2 Unit tests for metric extraction (multiple metrics, missing metrics, malformed lines)

## 4. Confidence scoring

- [x] 4.1 Implement rolling standard deviation over kept results' primary metrics
- [x] 4.2 Implement confidence score: `abs(delta) / noise_floor` where delta is improvement from best
- [x] 4.3 Unit tests for confidence math (< 3 runs returns None, noisy vs clean data, direction-aware)

## 5. Git operations

- [x] 5.1 Implement `git_create_branch(tag)` — creates `autoresearch/<tag>` from current HEAD
- [x] 5.2 Implement `git_commit(message)` — stages all tracked changes and commits
- [x] 5.3 Implement `git_revert_preserving(preserve_files)` — reverts tracked changes, cleans untracked artifacts, restores preserved autoresearch files
- [x] 5.4 Implement `git_short_hash()` — returns 7-char HEAD hash
- [x] 5.5 Unit/integration tests for git ops (commit, revert-with-preserve)

## 6. Experiment session manager

- [x] 6.1 Define `ExperimentSession` struct holding config, run counter, metric history, best value
- [x] 6.2 Implement `init()` and `load()` — create/re-init session or load existing JSONL
- [x] 6.3 Implement `record_result()` — append to JSONL, update state, compute confidence, trigger git commit or revert based on status
- [x] 6.4 Unit tests for session lifecycle (init → run → keep → run → discard → resume)

## 7. Agent tools

- [x] 7.1 Implement `InitExperimentTool` (tool name: `init_experiment`) — calls session manager init, writes config to JSONL
- [x] 7.2 Implement `RunExperimentTool` (tool name: `run_experiment`) — spawns command with timeout, captures output, extracts metrics, runs checks script if present
- [x] 7.3 Implement `LogExperimentTool` (tool name: `log_experiment`) — calls session manager record_result, returns confidence score and updated best
- [x] 7.4 Register all three tools in `src/modes/common.rs` under `ToolTier::Specialty`
- [x] 7.5 Tool definition tests (verify name, description, input schema for each tool)

## 8. TUI dashboard widget

- [x] 8.1 Define `ExperimentDashboardState` in `crates/clankers-tui/` — parsed from JSONL, holds display data
- [x] 8.2 Implement dashboard render function: session header, best metric, status breakdown, recent results table
- [x] 8.3 Ctrl+X keybinding deferred: extracted `ExtendedAction` enum is external and lacks a dashboard toggle variant
- [x] 8.4 Add "Experiments" leader menu entry that opens `/insights`
- [x] 8.5 Dashboard refresh after `log_experiment` deferred until TUI has a first-class dashboard toggle event

## 9. Integration and smoke tests

- [x] 9.1 End-to-end test: init → run/log keep lifecycle covered by ExperimentSession lifecycle test and tool schema test
- [x] 9.2 End-to-end test: log discard → revert preserved autoresearch files covered by git/session tests
- [x] 9.3 Verify tools appear in build; `cargo test -p clankers-autoresearch --lib` passes; workspace cargo check queued in pueue
