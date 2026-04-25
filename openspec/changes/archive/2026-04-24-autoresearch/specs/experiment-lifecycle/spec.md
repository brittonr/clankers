## ADDED Requirements

### Requirement: Initialize experiment session
The system SHALL provide an `init_experiment` tool that configures a new experiment session. It MUST accept: `name` (string, required), `metric_name` (string, required), `metric_unit` (string, optional), `direction` (string: "lower" or "higher", optional, default "lower"). It MUST write a config header line to `autoresearch.jsonl` in the working directory. If the file already exists with a config line, the tool MUST overwrite the config (to support re-initialization with a new baseline).

#### Scenario: First initialization
- **WHEN** the agent calls `init_experiment` with name="lr-sweep" and metric_name="val_bpb"
- **THEN** the tool creates `autoresearch.jsonl` with a single JSON line: `{"type":"config","name":"lr-sweep","metric_name":"val_bpb","metric_unit":null,"direction":"lower"}`

#### Scenario: Re-initialization
- **WHEN** `autoresearch.jsonl` already exists with a config line and 5 result lines
- **THEN** the tool replaces the config line, preserving all result lines

### Requirement: Run experiment with timing and metric extraction
The system SHALL provide a `run_experiment` tool that executes a shell command, captures wall-clock duration, captures stdout/stderr, and extracts metrics from `METRIC name=value` lines in stdout. The tool MUST accept: `command` (string, required), `timeout_seconds` (number, optional, default 600), `checks_timeout_seconds` (number, optional, default 300). Output MUST be truncated to the last 10 lines or 4KB (whichever is hit first), with full output saved to a temp file if truncated.

#### Scenario: Successful run with metrics
- **WHEN** the agent calls `run_experiment` with command="./autoresearch.sh"
- **AND** the script outputs "METRIC val_bpb=0.997" and "METRIC peak_vram_mb=45060"
- **THEN** the tool returns: exit code 0, wall-clock seconds, extracted metrics `{"val_bpb": 0.997, "peak_vram_mb": 45060.0}`, and truncated output

#### Scenario: Run exceeds timeout
- **WHEN** the command runs longer than `timeout_seconds`
- **THEN** the tool kills the process and returns a timeout error with partial output

#### Scenario: Run crashes
- **WHEN** the command exits with a non-zero exit code
- **THEN** the tool returns the exit code, wall-clock seconds, empty metrics, and truncated stderr/stdout

#### Scenario: Checks script exists and benchmark passes
- **WHEN** `autoresearch.checks.sh` exists in the working directory
- **AND** the benchmark command exits successfully
- **THEN** the tool runs `autoresearch.checks.sh` after the benchmark, with a separate timeout of `checks_timeout_seconds`
- **AND** checks execution time is excluded from the primary metric timing
- **AND** the result includes a `checks_passed` boolean field

#### Scenario: Checks script fails
- **WHEN** `autoresearch.checks.sh` exits with non-zero
- **THEN** the tool reports `checks_passed: false` with the last 80 lines of checks output

### Requirement: Log experiment result with git automation
The system SHALL provide a `log_experiment` tool that records an experiment result. It MUST accept: `commit` (string, required — short git hash), `metric` (number, required — primary metric value), `metrics` (object, optional — secondary metrics), `status` (enum: "keep", "discard", "crash", "checks_failed", required), `description` (string, required), `asi` (object, optional — agent-supplied information for future context), `force` (boolean, optional — skip confidence warning).

#### Scenario: Keep a successful result
- **WHEN** the agent calls `log_experiment` with status="keep"
- **THEN** the tool appends a result line to `autoresearch.jsonl` with an auto-incremented run number and ISO 8601 timestamp
- **AND** the current git state is committed (the branch advances)

#### Scenario: Discard a regression
- **WHEN** the agent calls `log_experiment` with status="discard"
- **THEN** the tool appends a result line to `autoresearch.jsonl`
- **AND** the tool reverts code changes via `git checkout -- .`
- **AND** autoresearch files (`autoresearch.jsonl`, `autoresearch.md`, `autoresearch.sh`, `autoresearch.checks.sh`, `autoresearch.ideas.md`, `autoresearch.config.json`) are preserved through the revert

#### Scenario: Log a crash
- **WHEN** the agent calls `log_experiment` with status="crash" and metric=0.0
- **THEN** the tool appends a result line and reverts, same as discard

#### Scenario: Log a checks failure
- **WHEN** the agent calls `log_experiment` with status="checks_failed"
- **THEN** the tool appends a result line and reverts, same as discard

#### Scenario: Confidence scoring after 3+ runs
- **WHEN** the session has 3 or more completed runs (any status)
- **THEN** the tool computes a confidence score: `abs(metric - best_kept) / noise_floor` where noise_floor is the standard deviation of the last 10 kept results' primary metrics
- **AND** the confidence score is included in the tool's response

### Requirement: Experiment session state
The system SHALL maintain experiment state that persists across tool calls within a session and can be reconstructed from `autoresearch.jsonl` on resume. State MUST include: config (name, metric, direction), run counter, best kept metric value, metric history for confidence scoring.

#### Scenario: Resume from existing JSONL
- **WHEN** `init_experiment` is called and `autoresearch.jsonl` already has result lines
- **THEN** the session state is reconstructed from the file: run counter continues from the last run number, best metric is derived from kept results, metric history is populated for confidence scoring

#### Scenario: State survives context reset
- **WHEN** the agent's context is compressed or a new agent attaches to the session
- **AND** the new agent calls `init_experiment` with the same parameters
- **THEN** the session resumes seamlessly from the JSONL file state
