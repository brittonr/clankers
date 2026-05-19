## Why

The batch runner already exposes reusable DTOs, an executor trait, run manifests, objective receipts, and resume behavior, but products lack a deterministic recipe and receipt validator for batch/eval use.

## What Changes

- Define `batch-eval-runner-kit` as a reusable brick with explicit boundaries, fixtures, and verification evidence.
- Add a narrow executable recipe, policy/manifest contract, generated inventory, or receipt rail as appropriate for this surface.
- Preserve FCIS boundaries: reusable bricks expose typed contracts and deterministic evidence, while shell/runtime I/O remains product-owned or app-edge.

## Capabilities

### New Capabilities
- `batch-trajectory-runner`: Batch Eval Runner Kit as a composable product-facing brick.

### Modified Capabilities
- Existing Clankers docs/tests should reference this brick only after deterministic fixtures and receipts prove the boundary.

## Impact

- **Files**: Expected changes are scoped to the named source anchors, examples, docs, policy, scripts, and focused tests for this brick.
- **APIs**: Public API changes must be minimal and typed; avoid promoting shell-owned internals into green SDK crates.
- **Dependencies**: New generic bricks must not add daemon, TUI, provider discovery, OAuth store, plugin supervisor, Matrix, or iroh dependencies to green SDK crates.
- **Testing**: Validate this change with `openspec validate brick-03-batch-eval-runner-kit --strict --json`, the focused recipe/checker, `git diff --check`, and any affected acceptance rail.
