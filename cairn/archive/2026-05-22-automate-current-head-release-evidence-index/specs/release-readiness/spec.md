# automated-current-head-release-evidence-index Specification

## Purpose
Define the local current-HEAD release evidence index rail that composes Clankers Git/lifecycle state with existing test-harness receipts without overclaiming readiness.

## Requirements

### Requirement: Harness evidence-index command [r[automated-current-head-release-evidence-index.harness-command]]
The repository MUST expose a developer-facing `./scripts/test-harness.sh evidence-index` mode that delegates to a Rust-owned evidence-index helper.

#### Scenario: Harness lists the evidence-index mode [r[automated-current-head-release-evidence-index.harness-command.listed]]
- GIVEN a developer runs `./scripts/test-harness.sh list`
- WHEN the harness prints available modes
- THEN it includes `evidence-index`
- AND it describes the mode as an index over existing local harness receipts rather than a replacement for running readiness profiles

#### Scenario: Harness delegates to Rust helper [r[automated-current-head-release-evidence-index.harness-command.delegates]]
- GIVEN a developer runs `./scripts/test-harness.sh evidence-index`
- WHEN the mode executes
- THEN the harness invokes the Rust-owned current-head release evidence helper
- AND the normal harness receipt records the delegated command and result

### Requirement: Current-head index generator [r[automated-current-head-release-evidence-index.generator]]
The evidence helper MUST gather current Git state, lifecycle state, and local harness receipts into a deterministic evidence index.

#### Scenario: Generator records repository state [r[automated-current-head-release-evidence-index.generator.repo-state]]
- GIVEN the helper runs in a Clankers checkout
- WHEN it writes an index
- THEN the index records branch, HEAD, upstream status, tag/describe distance, dirty status, and active Cairn/Cairn change directories

#### Scenario: Generator selects latest valid receipt per mode [r[automated-current-head-release-evidence-index.generator.receipt-selection]]
- GIVEN multiple harness run receipts exist under the result directory
- WHEN the helper evaluates receipts
- THEN it selects at most one latest passed receipt per mode using deterministic finished-time/run-id ordering
- AND it reports missing or invalid receipt classes without treating them as readiness passes

### Requirement: Fail-closed safety [r[automated-current-head-release-evidence-index.fail-closed]]
The helper MUST fail closed by default on unsafe or ambiguous promotion inputs.

#### Scenario: Dirty tracked worktree blocks default index [r[automated-current-head-release-evidence-index.fail-closed.dirty]]
- GIVEN the tracked worktree has uncommitted changes
- WHEN the helper runs without an explicit development override
- THEN it exits nonzero
- AND it does not present the checkout as clean current-HEAD evidence

#### Scenario: Bad receipts are not selected [r[automated-current-head-release-evidence-index.fail-closed.bad-receipt]]
- GIVEN a receipt has failed steps, no passed steps, invalid JSON, or references a missing log/summary/results artifact
- WHEN the helper scans local harness receipts
- THEN that receipt is rejected
- AND the index reports the corresponding evidence class as missing or invalid rather than passed

### Requirement: Deterministic output artifacts [r[automated-current-head-release-evidence-index.outputs]]
The helper MUST write deterministic JSON and Markdown artifacts under ignored `target/release-evidence/current-head/`.

#### Scenario: JSON and Markdown artifacts are emitted [r[automated-current-head-release-evidence-index.outputs.artifacts]]
- GIVEN acceptable repository state and local receipts
- WHEN the helper completes
- THEN it writes `index.json` with schema `clankers.current_head_release_evidence_index.v1`
- AND it writes `index.md` with the same payload HEAD, selected receipts, missing evidence classes, and non-claims
