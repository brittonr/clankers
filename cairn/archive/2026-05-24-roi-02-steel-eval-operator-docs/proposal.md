# Change: Steel eval operator documentation

## Why

The `steel_eval` tool is now default-published, but operator-facing docs do not yet make the default tool surface, opt-out, authority boundary, and receipt review path easy to discover.

## What Changes

- Document default `steel_eval` availability and `steelEval.enabled = false` opt-out.
- Explain the pure default profile: no ambient host functions, no session capabilities, zero host-call budget.
- Point operators to receipt fields and focused verification commands without overclaiming mutation authority.

## Non-Goals

- No behavior changes.
- No new host functions or non-default profiles.
- No Steel turn-planning activation changes.
