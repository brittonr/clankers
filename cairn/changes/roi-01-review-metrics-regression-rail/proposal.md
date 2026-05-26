# Change: Review metrics regression rail

## Why

Review metrics show repeated stage-gate omissions dominating the backlog: tasks auto-fix omissions, deterministic-check omissions, prompt trace omissions, and design/spec prompt omissions recur across hundreds of findings. A repo-local rail should turn those repeated classes into deterministic fixtures before more one-off review repairs accumulate.

## What Changes

- Add a Clankers-local review metrics regression rail for the highest-count omission categories.
- Preserve a sanitized metrics snapshot as planning evidence without raw prompts, credentials, or private transcript data.
- Extend repo-owned checker fixtures/docs before changing generic Cairn/OpenSpec core.

## Non-Goals

- No generic Cairn/OpenSpec gate rewrite in this change.
- No raw review transcript, secret, credential, or provider payload publication.
- No auto-fixing active changes without explicit later drain work.
