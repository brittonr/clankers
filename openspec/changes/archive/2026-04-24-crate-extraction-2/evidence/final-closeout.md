# Crate Extraction 2 Final Closeout Evidence

Evidence-ID: ce2-final-closeout
Task-ID: final-closeout
Artifact-Type: implementation-evidence
Covers: reduced-scope-cleanup, router-source-unification, workspace-continuity
Created: 2026-04-24
Status: complete

## Router source unification

The extracted `clanker-message` repo depends on `clanker-router` as a git dependency, while this workspace keeps the local vendored router authoritative through the existing patch:

```text
$ rg '\[patch\."https://github.com/brittonr/clanker-router"\]|clanker-router = \{ path = "vendor/clanker-router"' Cargo.toml
clanker-router = { path = "vendor/clanker-router" }
[patch."https://github.com/brittonr/clanker-router"]
clanker-router = { path = "vendor/clanker-router" }
```

Cargo metadata resolves exactly one `clanker-router` package, and it is the vendored source:

```text
path+file:///home/brittonr/git/clankers/vendor/clanker-router#0.1.0
/home/brittonr/git/clankers/vendor/clanker-router/Cargo.toml
source=None
```

## Workspace continuity closeout

`crate-extraction-2` was split after the reduced-scope implementation had already landed. Some per-extraction validation was recorded historically in `tasks.md`; the Phase 2a `cargo test` note includes one pre-existing tmux flake, not an extraction regression. The final reduced-scope closeout uses the durable full-workspace green bundle:

- Phase 3 task: `cargo check && cargo nextest run` for all 10 TUI type reverse deps.
- Phase 4 task: `cargo check && cargo nextest run` for all 7 message reverse deps.
- Phase 5 task: `RUSTC_WRAPPER= cargo check && RUSTC_WRAPPER= cargo nextest run` full workspace green, 1129 passed on 2026-04-24.
- This session: `RUSTC_WRAPPER= cargo check --workspace` succeeded.
- This session: `nix build .#clankers -L --no-link` succeeded after staging the vendored source tree.

This is accepted as the historical continuity evidence for closing the already-implemented reduced-scope change. `crate-extraction-3` retains stricter per-extraction validation tasks for remaining future extractions.

## Generated docs

No generated API docs were refreshed in this closeout because this step only changed source provenance from sibling path to checked-in vendor path. The generated artifact affected by source provenance was `build-plan.json`, and it was regenerated with `unit2nix --workspace --force --no-check -o build-plan.json`.
