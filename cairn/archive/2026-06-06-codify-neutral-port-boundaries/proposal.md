# Change: Codify Neutral Port Boundaries

## Why

A decoupled Clankers component should describe its needs with typed requests, effects, and service traits rather than importing a concrete provider, tool runner, storage backend, hook engine, plugin host, or config path reader. Some seams already follow this pattern, but it is not yet documented as a reusable rule for future extractions.

## What Changes

- Define a neutral-port boundary rule for model, tool, storage, prompt, hook, skill, cost, cancellation, and runtime services.
- Add an inventory rail that distinguishes contract DTOs/traits from adapter implementations.
- Refactor at least one touched seam to emit DTOs/effects and receive host services by trait injection instead of constructing concrete shell dependencies inline.

## Impact

- **Files**: agent/controller/runtime host-service definitions, tool/provider adapters, embedded SDK docs, and architecture rails.
- **Testing**: focused seam tests, dependency-boundary inventories, FCIS shell-boundary tests, and aggregate SDK acceptance if public contract labels change.
