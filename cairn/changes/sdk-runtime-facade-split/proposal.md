# Change: Split the Host Runtime Facade Into Smaller SDK Kits

## Problem

`clankers-runtime` is useful, but it is not a small lego brick. Its public API currently spans sessions, prompt assembly, extension services, provider DTOs, events, ledger/resume, process jobs, Steel orchestration/runtime/tool substrate, dynamic runtime authorization, and service stores. That breadth makes it hard to advertise as an SDK surface without dragging unrelated concepts.

## Goals

- Classify `clankers-runtime` public modules as green SDK kit, yellow app-edge service, or red desktop compatibility.
- Split or feature-gate unrelated runtime surfaces into smaller crates/modules with explicit dependency boundaries.
- Keep the minimal engine-host path independent of the facade.
- Make runtime service defaults fail closed rather than discovering desktop globals.

## Non-goals

- Do not remove existing runtime APIs without migration notes.
- Do not block desktop Clankers from composing the full runtime facade.
- Do not move Steel/process/plugin surfaces into green SDK crates by default.

## Proposed scope

Create a runtime-facade inventory and split plan. The first slice should extract or isolate one coherent kit such as prompt/session services, provider service DTOs, or Steel orchestration, then update generated SDK inventory/support labels.

## Verification

Validation should include public API inventory checks, dependency denylist checks per kit, examples that import only the selected kit, and fail-closed service tests.
