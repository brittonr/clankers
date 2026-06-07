# Design: Formalize Compatibility API Boundaries

## Compatibility categories

- `optional-support`: supported when the host explicitly opts into the concern or feature.
- `compatibility-alias`: supported old import/name that points to a canonical replacement.
- `unsupported-internal`: public only because of current crate layout or desktop compatibility; not an SDK promise.

Every compatibility API must identify its owner adapter, opt-in mechanism, and fixture. Default green examples must avoid these APIs unless the example is specifically about compatibility.

## Rails

The generated SDK inventory remains the source of truth for support labels. Boundary rails should reject root reexports or green API signatures that expose compatibility-only DTOs without an owner receipt.
