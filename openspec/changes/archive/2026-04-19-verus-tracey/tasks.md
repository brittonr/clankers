# Tasks — Verus + Tracey

## Phase 1: Scaffold + Graggle Proofs

- [x] Pin verus in flake.nix inputs, add to devshell
- [x] Create `docs/requirements.md` (sync from openspec specs)
- [x] Create `.config/tracey/config.styx`
- [x] Run `tracey query status` — baseline should show all uncovered
- [x] Create `verus/lib.rs` module root
- [x] Create `verus/merge_spec.rs` — GraggleModel type, well_formed spec fn
- [x] Prove `r[merge.dag.sentinels]` — new() produces sentinels
- [x] Prove `r[merge.dag.reachability]` — from_text maintains reachability
- [x] Prove `r[merge.dag.acyclicity]` — from_text produces acyclic graph
- [x] Prove `r[merge.insert.preserves-dag]` — insert_vertex on well-formed → well-formed
- [x] Prove `r[merge.from-text.linear]` — linear chain structure
- [x] Prove `r[merge.delete.ghost]` — alive=false, edges unchanged
- [x] Prove `r[merge.order-independence]` — 2-way commutativity lemma
- [x] Annotate `crates/clankers-merge/src/graggle.rs` with `r[impl merge.*]`
- [x] Annotate `crates/clankers-merge/src/merge.rs` with `r[impl merge.order-independence]`
- [x] `tracey query uncovered` returns 0 for merge requirements

## Phase 2: Actor + Session Proofs

- [x] Create `verus/actor_spec.rs` — RegistryModel type
- [x] Prove `r[actor.link.bidirectional]`
- [x] Prove `r[actor.unlink.bidirectional]`
- [x] Prove `r[actor.exit.link-cleanup]`
- [x] Prove `r[actor.exit.monitor-cleanup]`
- [x] Prove `r[actor.name.unique]`
- [x] Annotate `crates/clankers-actor/src/registry.rs` with `r[impl actor.*]`
- [x] Create `verus/session_spec.rs` — TreeModel type
- [x] Prove `r[session.walk.path-valid]`
- [x] Prove `r[session.walk.root-anchored]`
- [x] Prove `r[session.walk.terminates]`
- [x] Prove `r[session.index.consistent]`
- [x] Annotate `crates/clankers-session/src/tree/mod.rs` with `r[impl session.*]`
- [x] Annotate `crates/clankers-session/src/tree/navigation.rs` with `r[impl session.walk.*]`
- [x] `tracey query uncovered` returns 0 for actor + session requirements

## Phase 3: Protocol Proofs + CI

- [x] Create `verus/protocol_spec.rs` — frame model
- [x] Prove `r[protocol.frame.roundtrip]` (modulo serde axiom)
- [x] Prove `r[protocol.frame.size-reject-write]`
- [x] Prove `r[protocol.frame.size-reject-read]`
- [x] Prove `r[protocol.frame.length-encoding]`
- [x] Annotate `crates/clankers-protocol/src/frame.rs` with `r[impl protocol.*]`
- [x] Add existing frame tests as `r[verify protocol.*]`
- [x] Create `scripts/verify.sh` — runs verus + tracey
- [x] Add `checks.verus-proofs` to flake.nix
- [x] Add `checks.tracey-coverage` to flake.nix (updated to fail on gaps)
- [x] `tracey query uncovered` returns 0 for all requirements
- [x] `tracey query untested` returns 0 for all requirements
- [x] `verus --crate-type=lib verus/lib.rs` passes
