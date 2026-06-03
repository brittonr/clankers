# Change: Split Stable Message Contracts From Clankers Transcript Internals

## Problem

`clanker-message` is part of the green SDK surface, but it also exposes Clankers-specific transcript internals: `AgentMessage`, `MessageId`, bash execution records, branch/compaction summaries, custom messages, timestamps, and random ID generation. That makes the message crate useful but polluted for SDK consumers.

## Goals

- Separate stable content/tool/usage/semantic-event contracts from desktop transcript/session storage types.
- Keep `AgentMessage` and shell-only transcript variants marked compatibility/internal until moved or renamed.
- Provide neutral transcript DTOs for embedders when needed.
- Update generated SDK inventory labels and migration notes.

## Non-goals

- Do not break existing session persistence without migration adapters.
- Do not remove `Content`, `ToolDefinition`, `Usage`, streaming deltas, or semantic events from the green surface.
- Do not require embedders to adopt Clankers message IDs or timestamps.

## Proposed scope

Create a message-contract inventory, define the stable SDK subset, and move or isolate one internal transcript family behind a non-green module or compatibility label. Update session/provider/controller adapters to use stable DTOs at reusable boundaries.

## Verification

Validation should include API inventory checks, serialization compatibility fixtures, SDK examples using only stable message contracts, and rails that reject shell transcript internals in green APIs.
