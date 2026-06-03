# Change: Make Controller Runtime Boundary Lego-Clean

## Problem

`clankers-controller` has a useful `ControllerRuntimeAdapter` seam, but production controller state still owns concrete `Agent`, `SessionManager`, search index, hook pipeline, daemon protocol events, and display compatibility DTOs. That makes controller orchestration hard to reuse as a lego brick and keeps SDK/product integrations tied to daemon-era assumptions.

## Goals

- Make production prompt/control execution flow through injectable runtime/session adapters.
- Keep command translation, core effect interpretation, runtime dispatch, persistence, continuation, and projection separately owned.
- Remove or narrow controller dependencies on concrete agent/session/db/provider/display/protocol types where neutral DTOs already exist.
- Preserve daemon attach behavior through explicit edge projections.

## Non-goals

- Do not remove daemon Unix/QUIC protocols.
- Do not change user-facing session command behavior.
- Do not promote controller transport modules into the generic embedded SDK.

## Proposed scope

Advance `ControllerRuntimeAdapter` from test seam to production assembly seam. Controller command code should own neutral lifecycle policy, while root/daemon adapters inject agent-backed or runtime-backed services. Remaining protocol/TUI projections stay in conversion modules with rails.

## Verification

Validation should include fake-runtime controller fixtures, agent-backed parity tests, source rails for projection ownership, and no-socket controller tests.
