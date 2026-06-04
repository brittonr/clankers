# crate-extraction-2

## Why

This change originally covered a second extraction pass for ten crates. After
landing the highest-leverage shared crates, the scope was split so the finished
work can stand on its own and the remaining extractions can continue in a new
change.

`crate-extraction-2` now records the completed shared-core slice:

1. **clankers-plugin-sdk** → `clanker-plugin-sdk`
2. **clankers-specs** → `openspec`
3. **openspec-plugin** — WASM plugin wrapper for `openspec`
4. **clankers-tui-types** → `clanker-tui-types`
5. **clankers-message** → `clanker-message`

These extractions carried the biggest fanout reduction in the workspace:
- plugin authoring moved to a standalone SDK repo
- spec tooling became a standalone library plus plugin
- the two highest-fanout shared type crates moved out of-tree

The remaining six extractions continue in `crate-extraction-3`.

## What Changes

- Extract `clankers-plugin-sdk` as the standalone `clanker-plugin-sdk` crate.
- Extract `clankers-specs` as `openspec` with a pure core and filesystem shell.
- Package the `openspec` pure core as the `openspec-plugin` WASM plugin.
- Extract `clankers-tui-types` as `clanker-tui-types` and migrate direct callers.
- Extract `clankers-message` as `clanker-message` while keeping router sources unified.
- Remove reduced-scope wrapper crates and refresh workspace metadata for these extractions.

## Scope

### In Scope

#### Shared SDK extraction
- **clankers-plugin-sdk** → `clanker-plugin-sdk`
  - preserve wasm32 support
  - preserve prelude and protocol re-exports
  - update downstream plugins to the git dependency

#### Spec engine extraction
- **clankers-specs** → `openspec`
  - split pure parsing/graph logic from filesystem I/O
  - expose `openspec::core` for wasm-compatible logic
  - keep `SpecEngine` and native convenience API in the root crate

#### Spec tool WASM plugin
- **openspec-plugin**
  - package the pure `openspec` logic as an Extism plugin
  - expose the five spec/change/artifact tools used by clankers
  - keep filesystem access in the host, not in WASM

#### High-fanout type extractions
- **clankers-tui-types** → `clanker-tui-types`
  - migrate all direct callers to the extracted crate
  - remove the thin wrapper once callers are updated
- **clankers-message** → `clanker-message`
  - keep the router dependency on the extracted `clanker-router`
  - migrate all direct callers and remove the wrapper

#### Verification and cleanup
- remove reduced-scope wrapper crates from the workspace
- update workspace metadata/docs references for the extracted crates
- verify full-workspace continuity after the reduced-scope migration

### Out of Scope

Moved to `crate-extraction-3`:
- **clankers-nix** → `clanker-nix`
- **clankers-matrix** → `clanker-matrix`
- **clankers-zellij** → `clanker-zellij`
- **clankers-protocol** → `clanker-protocol`
- **clankers-db** → `clanker-db`
- **clankers-hooks** → `clanker-hooks`

Still out of scope for this extraction pass:
- `clankers-agent-defs`
- `clankers-prompts`
- `clankers-skills`
- `clankers-procmon`
- `clankers-model-selection`
- `clankers-provider`
- core app/runtime crates (`agent`, `controller`, `config`, `tui`, `session`, `plugin`, `util`)

## WASM Plugin Assessment

Within the reduced scope, only `openspec` qualifies for plugin packaging:
- `openspec` has a pure parsing/graph core that can compile to `wasm32-unknown-unknown`
- its operations are useful as LLM-callable tools
- its interface fits stateless JSON request/response semantics

The other reduced-scope crates do not become plugins:
- `clanker-plugin-sdk` is the SDK itself
- `clanker-tui-types` and `clanker-message` are compile-time shared types, not runtime tools

## Approach

Keep the proven extraction protocol from phase 1 and apply it only to the
reduced-scope crates:

1. preserve history (or do a clean repo move where appropriate)
2. rename the crate into the `clanker-*` / standalone namespace
3. add standalone repo scaffolding (README, LICENSE, CI)
4. switch the workspace to a reproducible source dependency (git when published, checked-in vendor snapshot for `openspec` until then)
5. keep a thin re-export wrapper only while callers are migrating
6. remove wrappers once all direct callers use the extracted crate
7. verify the full workspace after each high-risk migration step

The split itself is part of the approach: completed shared crates stay in this
change, while the remaining leaf/infrastructure crates move to
`crate-extraction-3` so each change has one coherent completion boundary.
