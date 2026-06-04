## Why

Durable process jobs expose backend-neutral service, retention, notification, security, and receipt contracts, but project job profiles lack a checked copyable manifest and static validator.

## What Changes

- Define `process-job-profile-kit` as a reusable brick with explicit boundaries, fixtures, and verification evidence.
- Add a narrow executable recipe, policy/manifest contract, generated inventory, or receipt rail as appropriate for this surface.
- Preserve FCIS boundaries: reusable bricks expose typed contracts and deterministic evidence, while shell/runtime I/O remains product-owned or app-edge.

## Capabilities

### New Capabilities
- `durable-process-jobs`: Process Job Profile Kit as a composable product-facing brick.

### Modified Capabilities
- Existing Clankers docs/tests should reference this brick only after deterministic fixtures and receipts prove the boundary.

## Impact

- **Files**: Expected changes are scoped to the named source anchors, examples, docs, policy, scripts, and focused tests for this brick.
- **APIs**: Public API changes must be minimal and typed; avoid promoting shell-owned internals into green SDK crates.
- **Dependencies**: New generic bricks must not add daemon, TUI, provider discovery, OAuth store, plugin supervisor, Matrix, or iroh dependencies to green SDK crates.
- **Testing**: Validate this change with `openspec validate brick-10-process-job-profile-kit --strict --json`, the focused recipe/checker, `git diff --check`, and any affected acceptance rail.
