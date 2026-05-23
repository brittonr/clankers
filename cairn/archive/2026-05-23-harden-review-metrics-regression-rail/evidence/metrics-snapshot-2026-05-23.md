# Metrics Snapshot: review-gate omission hardening

Artifact-Type: metrics-snapshot
Change: harden-review-metrics-regression-rail
Captured-From: `review_metrics_promotions --cwd /home/brittonr/git/clankers --min 2 --limit 10`
Captured-At: 2026-05-23
Sanitization: counts, categories, source/stage labels, and safe file/path examples only; no credentials, tokens, account identifiers, raw hidden prompts, provider payload bodies, or private transcript dumps.

## Dominant repeated findings

1. `omission|tasks|auto-fix` — count `491`
   - Sources: `openspec-gate=488`, `done-review=3`
   - Stages: `tasks=488`, `auto=3`
   - Representative safe themes:
     - Task ledgers mention broad request/SSE/auth work but omit exact normalized stream boundaries such as `MessageStart`, `ContentBlockStop`, `MessageDelta`, and `MessageStop`.
     - Task ledgers mention request conversion but omit explicit `text={"verbosity":"medium"}` default/override behavior.
     - Task ledgers mention provider-scoped auth but omit making the requested `openai-codex` account active after login.

2. `omission|tasks|deterministic-check` — count `151`
   - Sources: `openspec-gate=151`
   - Stages: `tasks=151`
   - Representative safe themes:
     - Deterministic request-fixture coverage omits entitlement probe retry/refresh paths.
     - Probe fixtures omit absence checks for normal-request-only transport headers.
     - Raw SSE seam coverage omits `response.function_call_arguments.delta` even when tool-call delta behavior is required.

3. `omission|tasks|prompt` — count `132`
   - Sources: `openspec-gate=125`, `done-review=7`
   - Stages: `tasks=120`, `auto=7`, `proposal=3`, `design=2`
   - Representative safe themes:
     - Discovery visibility/model-resolution requirements are not traced into explicit verification tasks.
     - Omitted-provider Anthropic default behavior is promised but not regression-tested.
     - Provider-scoped pending OAuth state isolation is required but not traced to a task.

4. `omission|design|prompt` — count `79`
   - Sources: `openspec-gate=79`
   - Stages: `design=79`
   - Representative safe themes:
     - Reasoning signature storage/reuse across later turns is required but not designed concretely.
     - Retry bounds and verification plans are described too vaguely.

5. `omission|spec|prompt` — count `40`
   - Sources: `openspec-gate=40`
   - Stages: `proposal=39`, `design=1`
   - Representative safe themes:
     - Omitted-provider Anthropic defaults, malformed account-claim behavior, and provider-scoped status behavior are promised but not encoded as explicit requirements/scenarios.

6. `omission|tasks|human` — count `36`
   - Sources: `openspec-gate=29`, `done-review=7`
   - Stages: `tasks=24`, `auto=7`, `design=4`, `proposal=1`
   - Representative safe themes:
     - A change is treated as ready even though mandatory sandbox, finish-line, or runtime/oracle proof is still open.
     - Required human/oracle evidence is not represented by an explicit `H#` task plus checked-in checkpoint artifact.

7. `incoherent|tasks|prompt` — count `28`
   - Sources: `openspec-gate=26`, `done-review=2`
   - Stages: `tasks=26`, `auto=2`
   - Representative safe themes:
     - Task text weakens a concrete design decision, for example retry counts/backoff/refresh limits.
     - Task ownership is ambiguous across auth layer and backend request layer.

## Selected category for this change

This change starts with the regression loop itself: every future metrics-derived review-gate hardening slice must preserve a safe metrics snapshot and then add deterministic negative/positive fixtures plus docs/wiring updates for the selected unsupported category.

The first implementation drain should inspect the existing checker categories and pick the highest-count unsupported gap, with special attention to `human` and `incoherent` task classes because the current checker already covers many deterministic task/design/spec omission examples.
