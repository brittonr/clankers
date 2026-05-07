## Context

The canonical `self-evolution-control` spec already requires offline runs, isolated candidates, human approval, explicit application, rollback, and safe receipts. The remaining production gap is evidence quality: a run can be mechanically safe while still relying on weak/noisy evals or non-observable orchestration.

## Goals / Non-Goals

**Goals:**
- Define reproducible eval corpus manifests and objective scoring boundaries.
- Require daemon/session-observable evaluation work for controlled dogfood profiles.
- Produce readiness reports that prevent overclaiming self-evolution maturity.
- Tighten promotion recommendations around regression budgets, unchanged-candidate detection, and human review evidence.

**Non-Goals:**
- Fully autonomous live mutation without approval.
- Open-ended internet benchmarks or non-reproducible LLM judge-only scoring.
- Replacing the existing apply/rollback safety model.

## Decisions

### 1. Eval corpora are explicit local manifests

**Choice:** Define a local manifest format listing targets, inputs, expected oracle type, scoring command, and redaction policy.

**Rationale:** Self-evolution needs stable objective evidence before it can claim improvement.

**Alternative:** Let each run provide arbitrary shell commands only. Rejected because arbitrary commands are hard to compare, audit, and aggregate.

### 2. Controlled dogfood uses daemon/session events

**Choice:** A productionization profile must drive work through the normal daemon/session substrate and record safe event receipts.

**Rationale:** Users need observability, interruption, and replay parity with regular Clankers sessions.

**Alternative:** Run all candidate work in a hidden local subprocess. Rejected because it bypasses the existing control-plane safety story.

### 3. Readiness labels are first-class outputs

**Choice:** Each run/report classifies status as `dry_run_only`, `controlled_dogfood`, `promotion_eligible`, or `blocked`.

**Rationale:** This prevents a green mechanical apply path from being mistaken for production-ready self-improvement.

**Alternative:** A boolean `recommended` flag only. Rejected because it is too coarse for operator decisions.

## Risks / Trade-offs

**Eval overfitting** → Require corpus provenance, train/eval split metadata where applicable, and regression fixtures.

**Noisy scores** → Require unchanged-candidate controls and minimum improvement thresholds before promotion eligibility.

**Event leakage** → Record safe event hashes/counts/status, not raw prompts, diffs, credentials, or artifact contents.
