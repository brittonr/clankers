# Change: Split Message Transcript SDK Defaults

## Problem

`clanker-message` is a green embedded SDK crate, but its default public root still re-exports Clankers transcript compatibility records such as `AgentMessage`, `MessageId`, and `generate_id`. Those records pull timestamp and random-ID concerns (`chrono`, `rand`, `hex`) into minimal embedded dependency graphs even though SDK docs classify transcript records as unsupported/internal.

## Goals

- Keep stable SDK message/content/usage/streaming/semantic-event contracts available by default.
- Move transcript compatibility records behind an explicit app-edge compatibility path or non-default feature.
- Ensure minimal embedded examples no longer require transcript IDs, wall-clock timestamps, or random ID generation from `clanker-message` defaults.
- Preserve desktop/session compatibility through an intentional migration path.

## Non-goals

- Do not remove persisted Clankers transcript compatibility support in this slice.
- Do not change provider/controller/session adapters beyond the compatibility imports needed for the split.
- Do not rename stable `content`, `contracts`, `streaming`, `tool_result`, or `semantic_event` contracts.

## Proposed scope

Introduce an explicit transcript compatibility boundary for `clanker-message`, update adapters that need transcript records to opt into that boundary, and strengthen SDK dependency/API rails so transcript internals cannot return to default green SDK imports.

## Verification

Focused validation should include message-contract boundary rails, embedded SDK dependency checks, minimal embedded example metadata proving transcript deps are absent from defaults, desktop transcript compatibility fixtures, `scripts/check-embedded-agent-sdk.rs`, Cairn gates, and `git diff --check`.
