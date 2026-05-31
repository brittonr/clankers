# Proposal: Drain Remaining Coupling Hotspots

## Problem

The first decoupling pass removed several obvious ownership problems, but architecture review still shows recurring coupling pressure in eight areas:

1. The root `clankers` crate still acts as a broad product-shell god crate with many internal dependencies and large mode/tool modules.
2. `clankers-agent` still imports concrete provider, router, DB, config, hook, procmon, and TUI DTO crates even though turn ports now exist.
3. Process-job behavior remains split across a large root `process` tool and a large runtime service module.
4. `clankers-controller::command` still mixes wire command handling, core reducer translation, agent mutation, persistence, and event projection.
5. Daemon session startup still constructs tools, agents, capability gates, persistence, hooks, plugin UI, and actor loops in one path.
6. Display/protocol DTO crates, especially `clanker-tui-types`, still leak inward as convenient domain models.
7. Provider/router compatibility still carries two provider abstractions and request/event conversion seams.
8. Some architecture rails still rely on brittle string anchors instead of typed or behavioral checks.

These are not one-shot bugs; they are the remaining seams most likely to re-accumulate product-specific policy in reusable modules.

## Proposed Change

Create a tracked drain plan for all remaining coupling hotspots, then remove them in small behavior-preserving slices. Each slice should either shrink a concrete dependency, move policy to a named owner, or replace a brittle rail with a typed/behavioral check.

The first drain slice will target the lowest-risk validation coupling: replace attach-parity source string anchors with more structured checks while preserving the existing parity docs contract. Later slices will address root shell thinness, agent dependency shrinkage, process-job decomposition, controller command seams, daemon actor construction, display DTO leakage, and provider/router convergence.

## Impact

- **Files**: `cairn/changes/drain-remaining-coupling-hotspots/**`, `tests/attach_parity_docs.rs`, root/dev manifests if typed test support is needed, then follow-on slices under `src/`, `crates/clankers-agent/`, `crates/clankers-controller/`, `crates/clankers-runtime/`, and `crates/clankers-provider/`.
- **Testing**: Cairn gates/validate, `cargo nextest` focused tests for each drained seam, `scripts/check-lego-architecture-boundaries.rs`, `./scripts/verify.sh`, and full nextest partitions before closeout.
