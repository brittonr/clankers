Artifact-Type: metrics-snapshot
Task-ID: H1
Covers: openspec-review-gates.metrics-derived-omission-prevention.safe-snapshot, openspec-review-gates.oracle-checkpoints
Captured-At: 2026-05-20T14:19:57Z
Source: review_metrics_promotions(cwd=/home/brittonr/git/clankers, min=2, limit=10)

## Sanitized repeated finding classes

- count=491, key=omission|tasks|auto-fix: repeated task omissions where spec/design-required contracts were not explicitly carried into tasks. Representative sanitized examples include missing stream-boundary coverage, missing exact default/override request-body checks, and missing active-account auth verification.
- count=151, key=omission|tasks|deterministic-check: deterministic request-fixture coverage omitted retry/refresh or event-delta cases required by proposal/design/spec artifacts.
- count=132, key=omission|tasks|prompt: discovery/model-resolution, omitted-provider defaults, and provider-scoped pending OAuth isolation were not traced to explicit verification tasks.
- count=79, key=omission|design|prompt: design artifacts omitted concrete storage/retry/verification contracts later required by specs.
- count=36, key=omission|tasks|human: repeated human/oracle checkpoint findings require explicit durable evidence rather than prose-only closeout.

## Safety review

This snapshot contains only aggregate counts, classification keys, and sanitized summaries from review-metrics output. It omits credentials, raw provider payloads, hidden prompts, tokens, tenant data, and live secret material.

## Scope decision

The first implementation should focus on high-confidence task-stage prevention fixtures and `H#` oracle-checkpoint handling, not a broad semantic reviewer replacement.
