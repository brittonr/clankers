# Proposal: harden spec omission gate

## Why

Review metrics still show repeated spec-stage omission findings after task-stage and design-stage review-gate hardening. The remaining high-value class is proposal or design promises that never become delta spec requirements/scenarios, which lets implementation tasks close without a durable accepted behavior contract.

Representative repeated omissions include omitted-provider Anthropic defaults, missing or malformed `chatgpt_account_id` claim behavior, and explicit provider-scoped status behavior.

## What Changes

- Add deterministic spec-stage omission categories to `scripts/check-cairn-review-gates.rs`.
- Add sanitized positive and negative fixtures for proposal/design promises that are or are not encoded in `spec.md`.
- Document spec-stage completeness guidance and diagnostic codes.
- Update the canonical `cairn-review-gates` Cairn spec through sync/archive.

## Non-goals

- No live provider, credential, qwen/aspen2, Codex, or network testing.
- No broad rewrite of the review-gate checker.
- No changes to product behavior outside deterministic review-gate hardening.

## Verification

- Run the review-gate fixture checker.
- Run formatting, mdBook, Cairn proposal/design/tasks gates, Cairn validate, and diff checks.
