# Review-gate fixture evidence

Evidence-ID: review-gate-fixtures
Artifact-Type: command-output-summary
Task-ID: V1
Covers: openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec, openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec-satisfied
Command: `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-openspec-review-gates.rs`
Status: PASS
Date: 2026-05-27

## Relevant output

```text
fixture negative-strong-constraint-spec-omission: Fail ["missing-strong-constraint-spec: strong proposal constraint \"forbidden github delivery path\" is missing or weakened in the delta spec", "missing-strong-constraint-spec: strong proposal constraint \"generated artifact hygiene\" is missing or weakened in the delta spec", "missing-strong-constraint-spec: strong proposal constraint \"required local verification\" is missing or weakened in the delta spec"]
fixture positive-strong-constraint-spec-coverage: Pass []
ok: openspec review-gate fixtures passed
```

## Scope

The command also re-ran the existing positive and negative fixtures for deterministic task contracts, design-stage omissions, spec-stage omissions, prompt traceability, auto-fix tasks, deterministic-check artifacts, and oracle checkpoint evidence.
