# crate-extraction

## Intent

Several crates in the workspace implement generic algorithms and infrastructure
that have nothing to do with coding agents. They happen to live here because
they were built here, but they'd be useful to anyone building actor systems,
merge tooling, LLM applications, or task scheduling on tokio.

Extracting them into standalone crates (separate GitHub repos) gives us:

- Reusable libraries for other projects without pulling in the full clankers tree
- Cleaner dependency boundaries — forces us to find and cut hidden coupling
- Independent git history per crate
- Smaller compile units for downstream consumers

## Scope

### In Scope

Six crates, ordered by extraction difficulty (easiest first):

1. **clankers-merge** -> `graggle` — order-independent merge via categorical pushout
2. **clankers-actor** -> `erlactor` — Erlang-style actors on tokio
3. **clankers-scheduler** -> standalone scheduling engine
4. **clankers-loop** -> standalone iteration/retry engine
5. **clankers-router** -> `llm-router` — multi-provider LLM routing with circuit breaker
6. **clankers-auth** -> `ucan-cap` — UCAN-inspired capability tokens on iroh identity

### Out of Scope

- **clankers-agent**, **clankers-controller**, **clankers-config** — too many
  workspace deps, inherently application-specific
- **clankers-tui**, **clankers-tui-types** — ratatui rendering tied to app state
- **clankers-message** — conversation message types are domain-specific
- **clankers-session** — Automerge storage coupled to message schema
- **clankers-db** — table schemas are clankers-specific
- **clankers-plugin** — host side depends on hooks + tui-types
- **clankers-protocol** — message types are clankers-specific (framing could
  be extracted but it's ~50 lines, not worth a crate)
- **clankers-hooks** — generic dispatch machinery but the HookPoint enum is
  small and domain-specific enough that extracting it adds overhead without
  much gain. Revisit if another project needs lifecycle hooks.
- **clankers-nix** — depends on local snix paths, not extractable until snix is

## Approach

Each extraction follows the same pattern:

1. Create a new GitHub repo
2. Move source files, preserving git history with `git subtree split`
3. Rename the crate (remove `clankers-` prefix, pick a good name)
4. Strip any remaining clankers references from source, docs, and tests
5. Add CI, README, LICENSE
6. In the clankers workspace: replace `path = "crates/clankers-foo"` with
   a git dep (`git = "https://github.com/brittonr/foo"`)
7. Add a re-export crate or `pub use` alias if the internal name matters
   for migration

The clankers workspace keeps compiling at every step. No big-bang migration.
