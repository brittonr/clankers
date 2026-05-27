# Sanitized review metrics snapshot — 2026-05-26

Source: `review_metrics_promotions --cwd /home/brittonr/git/clankers --min 2 --limit 10` run during ROI planning.

This snapshot preserves counts/classes only. It intentionally omits raw transcripts, credentials, provider payloads, hidden prompts, account identifiers, and secrets.

## Top repeated classes

| Rank | Count | Key | Sources / stages | Planning use |
| --- | ---: | --- | --- | --- |
| 1 | 491 | `omission|tasks|auto-fix` | mostly `cairn-gate`, tasks stage | First fixture family for task text that omits spec-required verification or boundary detail. |
| 2 | 151 | `omission|tasks|deterministic-check` | `cairn-gate`, tasks stage | Deterministic-check fixture family for retry/header/SSE/request-contract gaps. |
| 3 | 132 | `omission|tasks|prompt` | `cairn-gate` and `done-review` | Traceability fixture family for prompt/design/spec requirements not carried into tasks. |
| 4 | 79 | `omission|design|prompt` | `cairn-gate`, design stage | Later design-stage fixture family if task-stage rails do not reduce recurrence. |
| 5 | 40 | `omission|spec|prompt` | proposal/design/spec stages | Later spec-stage fixture family for proposal-to-spec compatibility boundaries. |

## Sanitized example classes

- Task text names a broad implementation area but omits exact stream/message boundary events required by spec.
- Task text asks for retry behavior but omits retry counts, refresh bounds, or deterministic request-fixture coverage.
- Task text covers discovery or auth generally but omits explicit hidden/visible/default-provider regression cases.
- Design text references an external/private oracle instead of freezing wire-level behavior in repo-owned artifacts.

## Selected first rail

Start with task-stage omission fixtures because they have the highest count and can be checked repo-locally without changing generic lifecycle core.
