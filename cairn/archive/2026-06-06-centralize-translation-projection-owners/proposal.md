# Change: Centralize Translation and Projection Owners

## Why

Transport, TUI, daemon, provider, and controller DTOs are intentionally different. Coupling returns when lower-level code rebuilds wire/display/provider shapes directly instead of going through the owner conversion module. The existing FCIS rails catch several cases; this change makes the ownership model explicit and broadens it to all projection seams.

## What Changes

- Inventory all protocol/display/provider/session projection owners and their allowed constructor sites.
- Add or extend constructor-only rails for wire DTOs, display DTOs, provider request DTOs, and controller core input/effect conversions.
- Refactor any touched code so reusable logic emits neutral domain events/receipts that projection adapters convert at the edge.

## Impact

- **Files**: `crates/clankers-controller/src/convert.rs`, `transport_convert.rs`, event processing, attach/daemon projection modules, provider adapter conversion, and FCIS boundary tests.
- **Testing**: FCIS shell-boundary tests, daemon/attach parity tests, provider/router request-shape tests, and protocol replay tests.
