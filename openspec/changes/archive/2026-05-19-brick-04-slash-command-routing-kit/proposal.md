## Why

Slash commands already have contributor and registry seams plus attach/local/daemon routing logic, but command extension remains stringly and easy to drift across docs, TUI, daemon attach, and prompt-template fallthrough.

## What Changes

- Define `slash-command-routing-kit` as a reusable brick with explicit boundaries, fixtures, and verification evidence.
- Add a narrow executable recipe, policy/manifest contract, generated inventory, or receipt rail as appropriate for this surface.
- Preserve FCIS boundaries: reusable bricks expose typed contracts and deterministic evidence, while shell/runtime I/O remains product-owned or app-edge.

## Capabilities

### New Capabilities
- `slash-command-composition`: Slash Command Routing Kit as a composable product-facing brick.

### Modified Capabilities
- Existing Clankers docs/tests should reference this brick only after deterministic fixtures and receipts prove the boundary.

## Impact

- **Files**: Expected changes are scoped to the named source anchors, examples, docs, policy, scripts, and focused tests for this brick.
- **APIs**: Public API changes must be minimal and typed; avoid promoting shell-owned internals into green SDK crates.
- **Dependencies**: New generic bricks must not add daemon, TUI, provider discovery, OAuth store, plugin supervisor, Matrix, or iroh dependencies to green SDK crates.
- **Testing**: Validate this change with `openspec validate brick-04-slash-command-routing-kit --strict --json`, the focused recipe/checker, `git diff --check`, and any affected acceptance rail.
