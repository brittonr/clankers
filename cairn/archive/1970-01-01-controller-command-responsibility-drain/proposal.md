# Change: Controller Command Responsibility Drain

## Problem

`crates/clankers-controller/src/command.rs` remains a broad command dispatcher that is still large enough to attract parsing, authorization, core input construction, runtime dispatch, persistence, continuation, and projection logic in one place. Existing converter modules help, but command responsibilities are not fully separated.

## Goals

- Map command responsibilities by owner.
- Extract at least one responsibility cluster from `command.rs` into a single-purpose module.
- Strengthen FCIS/source rails so projection and command translation stay centralized.

## Non-goals

- Do not change daemon/client protocol semantics.
- Do not move protocol DTO construction into new policy modules.
- Do not require live sockets for command seam tests.

## Proposed scope

Split a concrete command cluster, such as authorization/capability checks, persistence/resume mutations, runtime dispatch, or continuation policy, into a focused controller module with deterministic tests.

## Verification

Focused validation should include controller command tests, FCIS shell boundary rail, cargo check for controller, Cairn gates, and `git diff --check`.
