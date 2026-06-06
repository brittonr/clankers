# Design: Promote Transcript Compatibility Message Records

## Classification

The transcript records remain feature-gated behind `transcript-compat`, and default embedded SDK examples must continue to use `Content`, `ToolResult`, streaming, semantic events, and session-ledger DTOs instead of transcript records. The stability promotion therefore uses `optional-support`: supported when an embedding host explicitly opts into Clankers transcript compatibility, but not part of the default green message surface.

## Rail updates

`scripts/check-experimental-sdk-port-budget.rs` will accept promoted groups whose expected stability is any stable inventory label (`supported`, `optional-support`, or `compatibility-alias`) while retaining `experimental` and `absent` for retained and private groups. The message-contract boundary rail remains the guard that transcript records are not re-exported from the default crate root and are not used by green public APIs or embedded examples.

## Evidence

Focused evidence uses the existing `clanker-message` transcript compatibility roundtrip tests and the message-contract boundary rail. Policy evidence uses the embedded SDK inventory, experimental budget, and brick stability rails, followed by aggregate embedded SDK acceptance and Cairn gates.
