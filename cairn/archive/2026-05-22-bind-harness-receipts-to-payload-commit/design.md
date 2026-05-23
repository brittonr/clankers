# Design: payload-bound harness receipts

## Receipt capture
`./scripts/test-harness.sh` will capture payload Git state once at run start, before any test step executes. The emitted `results.json` will include a top-level `payload` object with:

- `commit`: `git rev-parse HEAD`
- `branch`: `git branch --show-current` with detached fallback
- `describe`: `git describe --tags --always --dirty`
- `tracked_dirty`: whether tracked files are dirty at harness start
- `upstream`: the symbolic upstream if present
- `ahead_behind`: ahead/behind counts if an upstream exists

The capture is informational evidence. Existing harness modes may still run on dirty worktrees unless their delegated tools impose stricter rules.

## Index verification
The current-head evidence-index helper will treat a receipt as payload-verified only when the receipt has `payload.commit` equal to the indexed HEAD and `payload.tracked_dirty=false`. Legacy receipts without a payload object remain selectable if otherwise valid, but are explicitly unverified.

Receipts from a different commit remain selectable as historical local evidence for their mode, but `payload_commit_verified=false` prevents overclaiming current-HEAD validation.

## Tests
Use small fixture receipts in Rust tests for the evidence helper where practical. Keep assertions deterministic and avoid depending on live historical `target/` contents.
