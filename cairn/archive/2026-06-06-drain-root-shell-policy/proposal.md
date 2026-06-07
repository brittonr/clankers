# Change: Drain Root Shell Policy

## Why

The root `clankers` crate still has the broadest internal dependency fan-in. That is acceptable only while the crate behaves as an application shell: parse CLI/TUI/daemon inputs, wire services, and project edge DTOs. Reusable policy still living in root modules makes future extractions harder and lets desktop assumptions leak back into SDK or runtime surfaces.

## What Changes

- Inventory root modules that currently own reusable policy, storage/schema mapping, provider construction, process-job behavior, plugin/session setup, or display/protocol projection.
- Assign each root dependency edge to an owner receipt that says whether it remains shell wiring, moves to a workspace crate, or becomes a focused adapter module.
- Drain at least one touched root policy cluster into its named owner, leaving root code as parsing, service assembly, or projection.
- Harden architecture rails so new root policy growth fails unless an owner receipt and convergence condition are added.

## Impact

- **Files**: root `src/` modes/tools/runtime-service modules, lego architecture ownership baseline, embedded SDK docs if labels move, and focused owner tests for any moved policy.
- **Testing**: root module focused tests, lego architecture boundary rail, FCIS shell-boundary rail for touched seams, Cairn validation/gates, `git diff --check`, and broader parity rails when user-visible behavior moves.
