# consume-basalt-steel-contracts

## Summary

Wire Clankers' `steel.host.plan_turn` path to consume Basalt's Steel contract DTO boundary instead of relying only on Clankers-local typed-plan strings and the external compile fixture. Clankers should build a Basalt `SteelEvaluationRequest`, validate it through Basalt, preserve Rust-owned authorization/fallback behavior, and record Basalt request/receipt hashes in the Steel turn-planning receipt evidence.

## Motivation

Basalt now defines the hardened UCAN/Nickel/Steel contract boundary and Clankers has a downstream fixture proving it can compile against that API. The next useful slice is product consumption: the real Steel turn-planning path should speak the Basalt DTO contract that UCAN/Nickel/Steel share, while Clankers remains the runtime/orchestration owner.

## Scope

- Add a narrow Basalt DTO bridge for `steel.host.plan_turn` in Clankers' runtime/agent path.
- Validate constructed Steel evaluation requests with Basalt before using Steel output.
- Bind Clankers receipts to Basalt request and receipt hashes without logging raw prompt/provider/script bodies.
- Add focused positive and negative tests for valid DTO construction, missing UCAN/session authority, malformed receipts, and fallback/block behavior.
- Keep Basalt as a contract crate and Clankers as the runtime owner.

## Non-goals

- Do not move Clankers runtime orchestration into Basalt.
- Do not grant Steel ambient filesystem/process/network/provider authority.
- Do not replace Clankers' Rust authorization, dynamic-runtime action envelope, fallback policy, or Steel runtime wrapper.
- Do not publish or tag a release.
