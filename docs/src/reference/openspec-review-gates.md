# OpenSpec Review Gates

Clankers keeps a project-local OpenSpec review-gate drift rail in:

```text
scripts/check-openspec-review-gates.rs
```

The checker uses sanitized fixtures in:

```text
scripts/fixtures/openspec-review-gates
```

Run it before closing changes that add or modify OpenSpec task-gate behavior:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-openspec-review-gates.rs
```

A routine Nix check is also exposed as `checks.<system>.openspec-review-gates`.

## Deterministic verification task guidance

When `proposal.md`, `design.md`, or delta specs require exact contracts, the task ledger must include explicit `V#` or implementation/verification tasks with `[covers=...]` and a concrete fixture, helper, command, golden file, script, or evidence path. Generic text such as "test the feature" is not enough.

The current deterministic contract categories are:

- request shape
- stream boundaries
- retry policy
- security/redaction policy
- receipt fields
- discovery visibility

The checker asserts representative diagnostics including:

- `missing-deterministic-request-shape-task`
- `missing-deterministic-stream-boundary-task`
- `missing-deterministic-retry-policy-task`

## Oracle checkpoint guidance

Repeated human-routed or oracle-routed review findings require an explicit `H#` task when mechanical checks cannot decide the claim. Do not close these with summary prose alone.

Each `H#` task must include `[covers=...]` and `[evidence=...]`. The evidence must be checked in under the change's `evidence/` directory and declare these fields:

```text
Artifact-Type: oracle-checkpoint
Task-ID:
Covers:
Reviewed-Evidence:
Decision:
Follow-Up:
```

The checker asserts representative diagnostics including:

- `missing-oracle-checkpoint-task`
- `invalid-oracle-checkpoint-evidence`
