# crate-extraction-2

## Intent

Phase 1 extracted 6 crates (graggle, clanker-actor, clanker-scheduler,
clanker-loop, clanker-router, clanker-auth). All done. The workspace still
has 24 crates, 12 of which have zero internal dependencies and could stand
on their own.

This second pass targets two groups:

1. **Leaf crates** with zero internal deps that are genuinely reusable
   outside clankers — same pattern as phase 1.
2. **High-fanout type crates** (tui-types, message) that 10+ other crates
   depend on. Extracting these would convert a large chunk of the remaining
   path deps into git deps.

We also evaluate each candidate for WASM plugin packaging. The answer for
most is no — they're compile-time infrastructure, not runtime tools — but
one (openspec) has a viable plugin path.

## Scope

### In Scope

Ten crates, grouped by extraction difficulty:

**Group A — Trivial leaf extractions (zero internal deps, 1–2 reverse deps):**

1. **clankers-plugin-sdk** → `clanker-plugin-sdk` — already has its own
   `[workspace]`, targets wasm32. Just needs a repo.
2. **clankers-nix** → `clanker-nix` — snix-based Nix integration. Leaf.
3. **clankers-matrix** → `clanker-matrix` — Matrix protocol bridge. Leaf.
4. **clankers-zellij** → `clanker-zellij` — P2P terminal sharing via
   Zellij + iroh. Leaf.

**Group B — Generic infrastructure (zero internal deps, moderate reverse deps):**

5. **clankers-protocol** → `clanker-protocol` — daemon↔client wire types.
6. **clankers-specs** → `openspec` — spec-driven development engine.
   Also gets a WASM plugin wrapper exposing spec tools to the LLM.
7. **clankers-db** → `clanker-db` — redb embedded database.
8. **clankers-hooks** → `clanker-hooks` — lifecycle hook dispatch.

**Group C — High-impact type crates (zero/minimal internal deps, many reverse deps):**

9.  **clankers-tui-types** → `clanker-tui-types` — UI event/action/block
    types. Zero internal deps, depended on by 10 crates.
10. **clankers-message** → `clanker-message` — conversation message types.
    1 internal dep (clanker-router, already extracted), depended on by 6.

### Out of Scope

- **clankers-agent-defs** — uses redb and is somewhat domain-specific.
  Revisit after clanker-db extraction if the redb dep can be inverted.
- **clankers-prompts**, **clankers-skills** — single `lib.rs` files with
  only a serde dep. Too thin for their own repos.
- **clankers-procmon** — depends on tui-types. Extract after tui-types.
- **clankers-model-selection** — depends on router + tui-types.
- **clankers-provider** — 3 internal deps, core integration layer.
- Core crates (agent, controller, config, tui, session, plugin, util) —
  too many deps, inherently application-specific.

### WASM Plugin Assessment

Each candidate was evaluated for packaging as a WASM plugin (extism,
wasm32-unknown-unknown target). A crate qualifies if:
(a) its dependency tree compiles to wasm32, and
(b) its functionality makes sense as an LLM-callable runtime tool.

| Crate | Compiles to wasm32? | Useful as LLM tool? | Verdict |
|---|---|---|---|
| plugin-sdk | Yes (it's the SDK) | N/A — it IS the SDK | Extract as lib only |
| nix | No (snix = native) | Yes | Extract as lib only |
| matrix | No (sqlite, TLS, crypto) | Marginal | Extract as lib only |
| zellij | No (iroh QUIC, tokio) | Marginal | Extract as lib only |
| protocol | No (tokio) | No — wire types | Extract as lib only |
| specs | Partially (petgraph ok, std::fs not) | Yes — spec ops | Extract as lib + WASM plugin |
| db | No (redb = mmap) | No — storage layer | Extract as lib only |
| hooks | No (tokio, async-trait) | No — dispatch infra | Extract as lib only |
| tui-types | Possibly (serde, chrono) | No — UI types | Extract as lib only |
| message | No (clanker-router) | No — conversation types | Extract as lib only |

Only `clankers-specs` gets a WASM plugin. The plugin wraps the pure
parsing/graph logic and exposes OpenSpec operations as LLM tools
(spec_list, change_create, change_verify, spec_context). Filesystem
access goes through the extism host.

## Approach

Same mechanical pattern as phase 1:

1. Create GitHub repo
2. Move source with `git subtree split`, preserving history
3. Rename crate, strip clankers references
4. Add CI, README, LICENSE
5. In workspace: replace path dep with git dep
6. Thin re-export wrapper during migration, remove later

New for phase 2: the openspec extraction also produces a WASM plugin
crate (`openspec-plugin`) that depends on the `openspec` library and
exposes tools via the clankers-plugin-sdk interface.
