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

When `proposal.md`, `design.md`, or delta specs require exact contracts, the task ledger must include explicit `V#` or implementation/verification tasks with `[covers=...]` and a concrete fixture, helper, command, golden file, script, or evidence path. Treat this as the required fixture/helper/command proof point; generic text such as "test the feature" is not enough.

The current deterministic contract categories are:

- request shape
- stream boundaries
- retry policy
- security/redaction policy
- receipt fields
- discovery visibility
- default/override subcontracts, such as `text={"verbosity":"medium"}` plus explicit caller override behavior
- active account persistence after provider-scoped login
- entitlement probe retry and refresh-retry fixtures, including probe-specific header omissions
- tool-call delta stream boundaries, especially raw `function_call_arguments.delta` to ordered input-JSON deltas

The checker asserts representative diagnostics including:

- `missing-deterministic-request-shape-task`
- `missing-deterministic-stream-boundary-task`
- `missing-deterministic-retry-policy-task`
- `missing-default-override-request-shape-task`
- `missing-active-account-task`
- `missing-entitlement-probe-retry-task`
- `missing-tool-call-delta-boundary-task`

## Design-stage completeness guidance

When `proposal.md` or delta specs require concrete design behavior, `design.md` must define the storage seam, policy bounds, and scenario-complete verification plan rather than using umbrella prose. The checker currently guards repeated omissions for:

- reasoning signature retention: where signatures are stored and how they are reused on a later turn
- retry policy bounds: exact 429/5xx retry count, `1s/2s/4s` backoff, exactly one 401 refresh retry, and one refresh cycle per request
- scenario-complete verification plan: proactive refresh, 401 retry, 429 retry, provider-scoped status, and discovery hiding cases

The checker asserts representative diagnostics including:

- `missing-reasoning-signature-design`
- `missing-retry-policy-design`
- `missing-verification-plan-design`

## Spec-stage completeness guidance

When `proposal.md` or `design.md` promises compatibility, error handling, status behavior, docs/help behavior, or acceptance boundaries, delta specs must encode those promises as explicit requirements/scenarios. The checker currently guards repeated omissions for:

- omitted-provider default behavior: commands without an explicit provider continue to use Anthropic defaults
- malformed account-claim behavior: missing or malformed `chatgpt_account_id` claim material is specified before use
- provider-scoped status behavior: explicit `status --provider openai-codex` behavior is specified rather than only `--all`

The checker asserts representative diagnostics including:

- `missing-omitted-provider-default-spec`
- `missing-malformed-account-claim-spec`
- `missing-provider-scoped-status-spec`

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
