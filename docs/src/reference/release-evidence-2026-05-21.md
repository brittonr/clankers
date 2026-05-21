# Release Evidence 2026-05-21

This bundle indexes durable evidence for the `internal-readiness-2026-05-21` checkpoint and the follow-on external-product dogfood run. It is an internal/trusted dogfooding evidence bundle, not a public unattended-production readiness claim.

## Checkpoint identity

- Readiness tag: `internal-readiness-2026-05-21`
- Tagged commit: `2fbc2de05a534e5de368f5ad1c8c07f38a4a55a1`
- Tagged commit subject: `docs: clarify harness evidence commit`
- Tagged commit date: `2026-05-20 20:44:31 -0400`
- Evidence bundle generated: `2026-05-21T14:51:49Z`
- Current checked commit during evidence recording: `b9e53853dc50a901a5d3164f08858356849768b1`
- Current checked commit subject: `Migrate lifecycle specs to Cairn`

## Repository state at evidence capture

- Clankers branch state: `## main...origin/main`
- Clankers worktree state before bundle capture: clean
- Current readiness scope: internal/trusted dogfood and external-product reasoning backend evidence only

## External-product dogfood evidence

Latest Tile/Remora external-product dogfood for this readiness line:

- State directory: `/home/brittonr/remora-operator-state/tile-clankers-release-20260521T004808Z`
- Status receipt: `/home/brittonr/remora-operator-state/tile-clankers-release-20260521T004808Z/review-schedule-last-status.json`
- Schedule report: `/home/brittonr/remora-operator-state/tile-clankers-release-20260521T004808Z/review-schedule-report.json`
- Rollup report: `/home/brittonr/remora-operator-state/tile-clankers-release-20260521T004808Z/review-schedule-rollup.json`
- Operator receipt bundle: `/home/brittonr/remora-operator-state/tile-clankers-release-20260521T004808Z/verification-receipts.json`
- Repository under review: `brittonr/tile`
- Change request: `#2`
- Mode: report-only execution with `review-schedule --execute --reasoning-execute`; no review comments were posted
- Health: `ok`
- Runs: `1`
- OK: `1`
- Failed: `0`
- Findings: `0`
- Posted comments: `0`
- Reasoning CLI: `clankers`
- Reasoning provider: `openai-codex`
- Reasoning model: `openai-codex/gpt-5.3-codex`
- Required fallback: `qwen36-aspen2`
- Primary attempts: `1`
- Fallback attempts: `0`
- Accepted primary: `1`
- Accepted fallback: `0`
- Accepted attempt index: `0`

Operator receipts in the state directory were clean:

- `dirty_state`: `clean`
- `verification_status`: `success`
- `commit_status`: `success`
- `push_status`: `success`
- `missing_receipts`: `false`
- `unverified_delegated_work`: `false`
- `dirty_after_mutation`: `false`
- `budget_exceeded_mutation_attempt`: `false`

The preserved verification receipt records:

- Receipt id: `tile-clankers-release-dogfood`
- Receipt status: `Success`
- Command summary: `review-schedule --execute --reasoning-execute completed with health ok; primary openai-codex/gpt-5.3-codex accepted and qwen36-aspen2 fallback unused`
- Remora verified commit: `e3c857c857f4aa170d8a5b6286aad5855db541be`
- Remora pushed remote/ref: `origin/main`

## Evidence interpretation

This dogfood run proves that Clankers was accepted as the reasoning backend for a real Tile review schedule run on the internal readiness line, with the `openai-codex/gpt-5.3-codex` primary accepted and the `qwen36-aspen2` fallback unused. The result is preserved as durable operator state under `/home/brittonr/remora-operator-state/tile-clankers-release-20260521T004808Z`.

It does not claim unattended public production readiness, broader live-provider stability, or mutation safety beyond the recorded report-only review schedule and clean operator receipts.
