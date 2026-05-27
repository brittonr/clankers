# Metrics snapshot: strong constraint spec omissions

- Artifact-Type: sanitized-review-metrics-snapshot
- Date: 2026-05-27
- Source command: `review_metrics action=promotions last=50 min=2`
- Records scanned: 50 of 661
- Selected category: `omission|spec|prompt`
- Count: 8
- Sources: `cairn-gate=8`
- Stages: `proposal=8`

## Safe representative examples

1. Generated artifact hygiene is not traceable to any delta requirement.
2. Local verification is weakened from required contract coverage to optional generic evidence.
3. A no-GitHub decision is weakened from forbidden to merely not required.

## Nearby categories

- `incoherent|spec|prompt` count 3: source-preservation or external-consumer dependency policy conflicts with the capability boundary.
- `omission|design|auto-fix` count 3: generated artifact refresh, preflight evidence preservation, or crate-local contract checks are under-specified in design.
- `omission|review|human` count 2: checked evidence is not fully reviewable, so future closeout may need explicit oracle-checkpoint artifacts.

## Selected prevention rail

Future lifecycle changes that state strong constraints in proposal or design artifacts must preserve those constraints in delta specs with equivalent normative strength and capability boundary. The review-gate checker should reject missing or weakened constraints with a category-specific diagnostic before tasks close.

## Sanitization boundary

This snapshot contains counts, classes, source stages, and sanitized behavior summaries only. It does not include credentials, tokens, account identifiers, raw hidden prompts, provider payload bodies, or private transcript dumps.
