## Why

Self-evolution has safe run/approve/apply/rollback receipts, but it is still not production-grade as an autonomous improvement loop. It needs stronger objective eval corpora, daemon-observable dogfood runs, promotion thresholds, regression guards, and a release-readiness label before it should be trusted beyond controlled experiments.

## What Changes

- **Eval corpus contract**: Define accepted objective datasets/transcripts for skills, prompts, tools, and code-path candidates.
- **Daemon dogfood orchestration**: Run candidate evaluation through normal daemon/session paths so users can observe, interrupt, and inspect events.
- **Promotion thresholds**: Require baseline-vs-candidate improvements, regression budgets, unchanged-candidate detection, and human approval before application.
- **Readiness reporting**: Emit a self-evolution readiness report that distinguishes dry-run, controlled-dogfood, and promotion-eligible states.

## Capabilities

### Modified Capabilities
- `self-evolution-control`: Self-evolution advances from safe local receipt mechanics toward objective, replayable, daemon-observable productionization gates.

## Impact

- **Files**: likely self-evolution command/receipt modules, daemon/session control integration, batch/eval runner seams, docs, and tests.
- **APIs**: may add eval corpus manifests, readiness reports, run profiles, and stricter recommendation fields.
- **Dependencies**: prefer existing batch trajectory/eval JSONL and session-control machinery before adding new eval dependencies.
- **Testing**: deterministic fake eval corpora, negative unchanged/regression fixtures, daemon/session event tests, CLI smokes for readiness reports, and docs checks.
