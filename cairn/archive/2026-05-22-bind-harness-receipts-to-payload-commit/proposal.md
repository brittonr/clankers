# Change: Bind harness receipts to payload commit

## Summary
Add Git payload commit metadata to every `./scripts/test-harness.sh` receipt so the current-head release evidence index can prove whether a selected receipt actually validated the indexed HEAD.

## Motivation
The current evidence index selects passed local receipts but must mark them `payload_commit_verified=false` because older harness receipts do not record the commit they validated. That is honest, but it weakens release-readiness evidence after a clean full harness: an operator cannot tell whether a selected receipt belongs to the current payload without comparing external context.

## Scope
- Extend harness `results.json` receipts with deterministic payload Git metadata captured at harness start.
- Teach the evidence-index helper to verify selected receipts against the indexed HEAD.
- Document the receipt metadata and the transition behavior for older receipts.
- Add focused positive/negative tests for matching, mismatched, and legacy receipts.

## Non-goals
- No remote CI integration.
- No tag movement or release publication.
- No rewriting historical receipts under `target/`.
