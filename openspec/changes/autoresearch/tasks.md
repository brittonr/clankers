## 1. Crate scaffolding

- [ ] 1.1 Create `crates/clankers-autoresearch/` with `Cargo.toml` (deps: serde, serde_json, tokio, chrono, thiserror/snafu)
- [ ] 1.2 Add `clankers-autoresearch` as a workspace member and dependency of the main binary crate

## 2. JSONL persistence and state

- [ ] 2.1 Define `ExperimentConfig` struct (name, metric_name, metric_unit, direction) with serde
- [ ] 2.2 Define `ExperimentResult` struct (type, run, commit, metric, metrics, status, description, asi, timestamp) with serde
- [ ] 2.3 Implement JSONL writer: append config line, append result line
- [ ] 2.4 Implement JSONL reader: parse config + results from existing file, reconstruct session state (run counter, best metric, metric history)
- [ ] 2.5 Unit tests for JSONL round-trip, re-init preserving results, resume from partial file

## 3. Metric extraction

- [ ] 3.1 Implement `METRIC name=value` line parser for stdout (handles int, float, scientific notation)
- [ ] 3.2 Unit tests for metric extraction (multiple metrics, missing metrics, malformed lines)

## 4. Confidence scoring

- [ ] 4.1 Implement rolling standard deviation over last N kept results' primary metrics
- [ ] 4.2 Implement confidence score: `abs(delta) / noise_floor` where delta is improvement from best
- [ ] 4.3 Unit tests for confidence math (< 3 runs returns None, noisy vs clean data, direction-aware)

## 5. Git operations

- [ ] 5.1 Implement `git_create_branch(tag)` — creates `autoresearch/<tag>` from current HEAD
- [ ] 5.2 Implement `git_commit(message)` — stages all tracked changes and commits
- [ ] 5.3 Implement `git_revert_preserving(preserve_files)` — `git checkout -- .` with stash/restore of autoresearch files
- [ ] 5.4 Implement `git_short_hash()` — returns 7-char HEAD hash
- [ ] 5.5 Unit/integration tests for git ops (commit, revert-with-preserve)

## 6. Experiment session manager

- [ ] 6.1 Define `ExperimentSession` struct holding config, run counter, metric history, best value
- [ ] 6.2 Implement `init()` — create or re-init session from config, load existing JSONL if present
- [ ] 6.3 Implement `record_result()` — append to JSONL, update state, compute confidence, trigger git commit or revert based on status
- [ ] 6.4 Unit tests for session lifecycle (init → run → keep → run → discard → resume)

## 7. Agent tools

- [ ] 7.1 Implement `InitExperimentTool` (tool name: `init_experiment`) — calls session manager init, writes config to JSONL
- [ ] 7.2 Implement `RunExperimentTool` (tool name: `run_experiment`) — spawns command with timeout, captures output, extracts metrics, runs checks script if present
- [ ] 7.3 Implement `LogExperimentTool` (tool name: `log_experiment`) — calls session manager record_result, returns confidence score and updated best
- [ ] 7.4 Register all three tools in `src/modes/common.rs` under `ToolTier::Specialty`
- [ ] 7.5 Tool definition tests (verify name, description, input schema for each tool)

## 8. TUI dashboard widget

- [ ] 8.1 Define `ExperimentDashboardState` in `crates/clankers-tui/` — parsed from JSONL, holds display data
- [ ] 8.2 Implement dashboard render function: session header, best metric, status breakdown, scrollable results table
- [ ] 8.3 Add Ctrl+X keybinding to toggle dashboard visibility in `crates/clankers-tui/`
- [ ] 8.4 Add "Experiments" leader menu entry that toggles dashboard
- [ ] 8.5 Wire dashboard refresh after `log_experiment` events (via `AgentEvent` bus)

## 9. Integration and smoke tests

- [ ] 9.1 End-to-end test: init → run (with a trivial script that prints METRIC lines) → log keep → verify JSONL and git state
- [ ] 9.2 End-to-end test: init → run → log discard → verify revert preserved autoresearch files
- [ ] 9.3 Verify tools appear in `cargo nextest run` and `cargo clippy -- -D warnings` passes
