# Change: Drain Agent Shell Coupling Behind SDK Ports

## Problem

`clankers-agent` still behaves like a desktop application shell: it owns provider objects, settings, database handles, hooks, prompt/skill discovery inputs, model-routing state, cost tracking, tool maps, event broadcast, and cancellation. The reusable engine/host bricks already exist, but an SDK user cannot embed the agent-shaped behavior without pulling those concrete Clankers systems with it.

## Goals

- Make the agent turn shell explicitly depend on narrow model, tool, prompt, storage, hook, skill, cost, and cancellation ports.
- Move concrete provider/config/DB/hook/skill/model-selection adapters to root, daemon, or runtime assembly edges.
- Preserve default desktop behavior through compatibility adapters while shrinking `clankers-agent` concrete dependencies.
- Add source/dependency rails that show the remaining concrete edges and their convergence conditions.

## Non-goals

- Do not remove the existing desktop `Agent::new` compatibility constructor in this slice unless all call sites are migrated.
- Do not rewrite provider backends, built-in tools, or daemon attach behavior.
- Do not promote `clankers-agent` itself into the green embedded SDK surface.

## Proposed scope

Introduce an SDK-facing agent-shell boundary that accepts explicit ports or service bundles and makes the current concrete `Agent` constructor a desktop adapter. The first implementation slice should inventory the current concrete fields, extract at least provider/model execution and one non-model service family behind ports, and update the lego architecture receipts so future coupling additions are intentional.

## Verification

Validation should include focused agent turn tests, adapter parity tests for desktop behavior, dependency/source rails rejecting new concrete imports in reusable turn modules, and Cairn gates/validate.
