# Design: Steel eval live tool discovery receipt

## Context

Unit tests prove registration, but a product-level receipt should prove an actual Clankers runtime/tool-list path sees `steel_eval` by default and hides it through opt-out or disabled-tool policy.

## Approach

- Prefer an existing CLI/daemon/tool-list seam that can run deterministically with fake/local provider setup.
- The receipt should include tool name presence/absence assertions and the settings/policy mode used.
- If live daemon paths are flaky, first land a deterministic runtime seam test and treat external live receipt as optional evidence.

## Verification

- Validate this Cairn package with repo-local/native Cairn validation.
- Run proposal, design, and tasks gates and inspect `valid`/`verdict` receipts.
- Run the implementation-specific verification named in `tasks.md` when draining this package.
