# Change: Move Controller Production Commands Through Runtime Adapter

## Problem

`clankers-controller` has a `ControllerRuntimeAdapter` seam and fake-service fixtures, but production command handling still mutates `Agent` directly in `SessionController::handle_command` and prompt execution. As long as command policy owns concrete `Agent` calls, the controller remains coupled to the agent crate and fake-service coverage is a parallel path rather than the production path.

## Goals

- Make production prompt and control handling go through the same `ControllerRuntimeAdapter` contract used by fake-service tests.
- Move concrete `Agent` prompt/control operations into a named production adapter built by the root/daemon shell.
- Keep command authorization, reducer input construction, continuation policy, and event projection in the controller.
- Shrink direct `Agent` mutation in `crates/clankers-controller/src/command.rs` to adapter wiring or compatibility shims.

## Non-goals

- Do not rewrite the agent turn loop or daemon actor loop in this slice.
- Do not remove all `clankers-controller -> clankers-agent` references until the production adapter can live fully outside the controller crate.
- Do not change daemon, local attach, remote attach, or embedded session behavior.

## Proposed scope

Introduce a production runtime adapter that wraps the current agent/event receiver behavior behind `ControllerRuntimeAdapter`. Convert one prompt/control path at a time so the existing fake adapter exercises the same controller command code as daemon production mode.

## Verification

Validation should include fake-runtime controller fixtures, daemon/attach parity fixtures for prompt, abort/reset, thinking, disabled tools, and session id propagation, the FCIS shell-boundary rail, the lego architecture rail, Cairn gates, and `git diff --check`.
