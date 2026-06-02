# Change: Process Root Projection Final Drain

## Problem

`src/tools/process.rs` is thinner after backend extraction, but it is still the largest root tool file and still owns native service shells, `ProcessEntry`, user-facing native receipt projection, and a large test module. That keeps native process policy close to the product shell and makes future process work likely to land in the root file again.

## Goals

- Move native service and entry ownership into focused process modules.
- Leave `ProcessTool` responsible only for JSON/tool action projection, backend selection, and typed receipt envelope formatting.
- Update architecture rails to reject native service or entry policy returning to `src/tools/process.rs`.

## Non-goals

- Do not change the user-facing process tool schema.
- Do not require pueue, systemd, or live daemon services for tests.
- Do not create a new process-job crate unless the focused module split exposes a clear crate boundary.

## Proposed scope

Extract the remaining native service and entry policy from the root process tool into `src/tools/process/native.rs` or a sibling service module, move native-specific tests with the owner, and keep root tests limited to parser/projection parity.

## Verification

Focused validation should include native service fixtures, process tool parity tests, `cargo check -p clankers --tests`, process boundary rails, Cairn gates, and `git diff --check`.
