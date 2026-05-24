# Change: Default Steel eval tool publication

## Why

`steel_eval` now has a reviewed Rust-owned tool shell, pure default profile, zero ambient host authority, deterministic receipts, and disabled-tool parity. Keeping that safe profile hidden unless users write explicit settings makes Steel feel like an installed but non-default capability.

We want Clankers' ordinary settings defaults to publish the safe `steel_eval` tool by default while retaining fail-closed behavior for host authority, oversized inputs, unsupported profiles, and explicit disabled-tool policy.

## What Changes

- Make `Settings::default().steel_eval.enabled` true for the pure default profile.
- Keep the default profile host-authority-free: no session capabilities, no host functions, zero host-call budget.
- Preserve explicit opt-out through `steelEval.enabled = false` and the normal disabled-tool filter.
- Update focused tests so default settings publish `steel_eval`, disabled settings omit it, and disabled-tool filtering still removes it.

## Non-Goals

- No ambient host functions or mutation authority.
- No change to Steel turn-planning default activation.
- No runtime fallback that bypasses the existing Steel wrapper.
- No broad tool-catalog redesign.
