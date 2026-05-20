# OpenSpec Review Gate Fixture Diagnostics

Command:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-openspec-review-gates.rs
```

Result: PASS

Fixtures covered:

- `negative-vague-deterministic`: rejects generic testing tasks when design/spec artifacts require deterministic request shape, stream boundary, and retry policy contracts.
- `positive-fixture-backed-deterministic`: accepts task lines that name concrete fixtures/helpers/commands and map them with `[covers=...]`.
- `negative-oracle-missing`: rejects repeated human/oracle findings without an `H#` task.
- `negative-oracle-prose-only`: rejects prose-only evidence that lacks `Artifact-Type: oracle-checkpoint` and required oracle fields.
- `positive-oracle-checkpoint`: accepts checked-in oracle checkpoint evidence with `Artifact-Type`, `Task-ID`, `Covers`, `Reviewed-Evidence`, `Decision`, and `Follow-Up`.

Diagnostic families asserted:

- `missing-deterministic-request-shape-task`
- `missing-deterministic-stream-boundary-task`
- `missing-deterministic-retry-policy-task`
- `missing-oracle-checkpoint-task`
- `invalid-oracle-checkpoint-evidence`
