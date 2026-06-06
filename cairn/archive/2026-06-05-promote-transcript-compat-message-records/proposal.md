# Change: Promote Transcript Compatibility Message Records

## Why

The experimental SDK budget now only carries the `clanker-message` transcript compatibility records for user, assistant, and tool-result messages. These records are intentionally outside the default green message contract, but they are already guarded by the non-default `transcript-compat` feature and exercised by compatibility tests. Keeping them `experimental` makes the budget non-zero without identifying new work.

## What Changes

- Classify `UserMessage`, `AssistantMessage`, `ToolResultMessage`, and their public fields as optional supported compatibility records.
- Update the experimental budget rail so promoted optional-support groups are allowed and the expected experimental row count reaches zero.
- Refresh the generated embedded SDK inventory and brick stability policy after the stability change.

## Impact

- **Files**: `docs/src/generated/embedded-sdk-api.md`, `policy/embedded-lego/*.json`, `scripts/check-experimental-sdk-port-budget.rs`, `scripts/check-message-contract-boundary.rs`, and Cairn evidence.
- **Testing**: `cargo test -p clanker-message --features transcript-compat`, message-contract boundary rail, experimental-budget rail, brick stability rail, and aggregate embedded SDK acceptance.
