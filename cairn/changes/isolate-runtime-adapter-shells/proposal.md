# Change: Isolate Runtime Adapter Shells

## Why

`clankers-runtime` is intentionally a yellow application-edge facade today, but reusable contracts and desktop adapter implementations are still close enough that public API inventories grow whenever shell services add DTOs. A clearer split would let embedders depend on green contracts without importing desktop runtime adapters.

## What Changes

- Define a runtime adapter-shell boundary that separates green contract modules from yellow desktop service implementations.
- Move or classify provider/auth/plugin/process/prompt/session/storage default implementations as adapter-only.
- Update runtime facade docs and inventories so every public runtime row is either green contract, yellow host-injection surface, or desktop adapter.

## Impact

- **Files**: `crates/clankers-runtime/src/*`, runtime facade policy/inventory, embedded docs, root runtime service wiring, and adapter parity rails.
- **Testing**: runtime facade boundary rail, runtime extension service matrix, root-controller-runtime adapters rail, embedded SDK acceptance, and Cairn gates.
