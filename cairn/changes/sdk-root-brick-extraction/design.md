# Design: Extract Root Tool and Mode Policy Into SDK Bricks

## Summary

The root crate is allowed to be big as a product shell, but it should not be the only owner of reusable tool/runtime/slash policy. This change makes root policy ownership explicit and extracts one representative brick.

## Current coupling points

- `src/tools/` contains agent-visible tool adapters plus backend/service policy for many tools.
- `src/runtime_services.rs` bridges desktop paths to runtime services and provider/plugin/auth adapters.
- `src/slash_commands/` and `src/modes/session_command_policy.rs` own reusable command-effect decisions near app-edge protocol projection.
- `src/modes/daemon/` mixes assembly, actor, plugin, protocol, and runtime concerns despite recent splits.

## Decisions

### 1. Root may wire, not own reusable policy

Root modules can parse CLI input, assemble services, register tools, and project to user-facing surfaces. Reusable policy belongs in workspace crates or clearly named adapter modules with receipts.

### 2. Extract one brick per slice

The migration should choose one policy cluster with clear tests. Candidates include process/tool backend policy, slash command effects, desktop runtime services, or daemon session assembly.

### 3. Receipts are required for remaining root edges

If root keeps policy temporarily, the lego baseline must say why and where it should move.

## Validation plan

- Root policy inventory with module cluster, owner, and target crate/module.
- Focused tests for the extracted brick independent of the root binary.
- Root parity tests for CLI/TUI/daemon behavior.
- Architecture rails distinguishing wiring-only imports from policy ownership.
