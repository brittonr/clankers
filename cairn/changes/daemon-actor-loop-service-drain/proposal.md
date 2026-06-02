# Change: Daemon Actor Loop Service Drain

## Problem

`src/modes/daemon/agent_process.rs` still combines actor-loop multiplexing with plugin UI draining, tool-list synchronization, schedule handling, prompt RPC collection, cancellation, and controller event forwarding. That makes the daemon actor hard to reason about and makes session-runtime policy depend on actor-loop details.

## Goals

- Move one daemon actor-loop responsibility into a focused service or tick adapter.
- Keep actor loop code as polling/multiplexing over assembled services.
- Add socketless tests for the extracted service.

## Non-goals

- Do not change daemon protocol frames.
- Do not require a live daemon socket or external plugin process for focused tests.
- Do not split every actor concern in one change.

## Proposed scope

Select a responsibility cluster, preferably plugin UI event draining, tool-list sync, schedule dispatch, or prompt RPC collection, and move it into an assembled daemon service with deterministic tests.

## Verification

Focused validation should include daemon actor/service tests, relevant attach/daemon parity tests, architecture rails, Cairn gates, and `git diff --check`.
