# batch-trajectory-runner Specification

## Requirements

### Requirement: Batch eval kit validates deterministic manifests and resume receipts

The batch-eval-runner-kit SHALL keep trajectory output, deterministic resume manifests, and redacted metadata in sync.

#### Scenario: batch-eval-runner-kit.boundary
- GIVEN local batch inputs are validated
- WHEN remote or unsafe paths are requested
- THEN validation MUST fail closed before execution.

#### Scenario: batch-eval-runner-kit.evidence
- GIVEN eval JSONL output is rendered
- WHEN the receipt is generated
- THEN deterministic resume metadata MUST be available for replay.

#### Scenario: batch-eval-runner-kit.drift
- GIVEN source, docs, or receipt behavior changes
- WHEN `scripts/check-batch-eval-runner-kit.rs` runs
- THEN the kit MUST fail until all linked artifacts are updated together.
