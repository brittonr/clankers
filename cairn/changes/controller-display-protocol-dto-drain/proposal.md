# Change: Drain Controller Display and Protocol DTO Coupling

## Problem

`clankers-controller` still imports display/protocol DTOs in reusable command and auto-test paths, including `clanker_tui_types::ThinkingLevel`, `LoopDisplayState`, and direct protocol event construction near command policy. Conversion modules own much of the projection already, but remaining inward DTO use keeps display/transport types acting as controller domain state.

## Goals

- Replace display-only thinking and loop state inputs in controller policy with core or neutral DTOs.
- Keep TUI/protocol constructors in explicit projection modules such as `convert.rs` and `transport_convert.rs`.
- Move command-path protocol event construction behind semantic/domain event projection where practical.
- Add source rails that distinguish allowed projection adapters from forbidden inward display/protocol state.

## Non-goals

- Do not remove `clanker-tui-types` or `clankers-protocol` from projection modules in this slice.
- Do not change daemon wire frames or TUI events.
- Do not rewrite all controller command handling; focus on DTO ownership and projection seams.

## Proposed scope

Start with thinking-level parsing and auto-test loop display sync: introduce neutral/core request DTOs, update attach/TUI adapters to project into them, and keep TUI/protocol DTOs at the edge. Then inventory remaining direct `DaemonEvent` constructors in command policy and move one user-visible branch through semantic projection.

## Verification

Validation should include controller unit fixtures for thinking/loop sync, conversion/projection tests, attach parity tests, FCIS constructor-ownership rails, lego architecture rails, Cairn gates, and `git diff --check`.
