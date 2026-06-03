# Design: Split the Host Runtime Facade Into Smaller SDK Kits

## Summary

`clankers-runtime` should be a composition layer, not one monolithic SDK contract. This change defines kit boundaries so embedders can choose prompt assembly, session ledger, provider services, Steel orchestration, or extension services independently.

## Current coupling points

- `src/lib.rs` re-exports many unrelated modules and public types.
- `session.rs` drives `clankers-engine-host` but also allocates UUIDs/timestamps and maps runtime model/tool adapters.
- `services.rs` defines settings/auth/cache/session/project/skill/plugin/checkpoint/provider/extension services together.
- Steel orchestration/runtime/tool substrate and process job profiles share the same facade namespace.

## Decisions

### 1. Runtime facade is yellow until split

The green path remains engine/engine-host/tool-host/adapters/message/core. Runtime can be host-facing, but its sub-surfaces need per-kit labels before being advertised.

### 2. Split by product capability

Potential kits include prompt-source services, session ledger/resume, neutral provider router service DTOs, extension runtime services, Steel orchestration, process-job profile, and dynamic runtime authorization.

### 3. Defaults fail closed

A kit must not probe filesystem paths, global auth, plugins, or network services unless the app injects an explicit desktop adapter or policy.

## Selected drain slice

Selected kit: `session-ledger-resume`. `scripts/check-runtime-facade-split.rs` records the selected kit boundary and support labels. The kit's green DTOs live in `crates/clankers-runtime/src/ledger.rs`, host-owned execution runs through `Runtime::resume_session` in `session.rs`, and host service defaults in `services.rs` fail closed through `DisabledSessionStore` rather than discovering desktop globals.

Out-of-scope surfaces for this selected kit are provider/router/auth/plugin/TUI/daemon/process/Steel surfaces unless a host injects them through an explicit service adapter.

## Validation plan

- Runtime public API/module inventory with support labels.
- Dependency graph checks per kit.
- Example or fixture for one split kit.
- Fail-closed tests for missing services and disabled filesystem/global discovery.
