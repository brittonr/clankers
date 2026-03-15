# Tasks — Verus + Tracey

## Phase 1: Scaffold + Graggle Proofs

- [ ] Pin verus in flake.nix inputs, add to devshell
- [ ] Create `docs/requirements.md` (sync from openspec specs)
- [ ] Create `.config/tracey/config.styx`
- [ ] Run `tracey query status` — baseline should show all uncovered
- [ ] Create `verus/lib.rs` module root
- [ ] Create `verus/merge_spec.rs` — GraggleModel type, well_formed spec fn
- [ ] Prove `r[merge.dag.sentinels]` — new() produces sentinels
- [ ] Prove `r[merge.dag.reachability]` — from_text maintains reachability
- [ ] Prove `r[merge.dag.acyclicity]` — from_text produces acyclic graph
- [ ] Prove `r[merge.insert.preserves-dag]` — insert_vertex on well-formed → well-formed
- [ ] Prove `r[merge.from-text.linear]` — linear chain structure
- [ ] Prove `r[merge.delete.ghost]` — alive=false, edges unchanged
- [ ] Prove `r[merge.order-independence]` — 2-way commutativity lemma
- [ ] Annotate `crates/clankers-merge/src/graggle.rs` with `r[impl merge.*]`
- [ ] Annotate `crates/clankers-merge/src/merge.rs` with `r[impl merge.order-independence]`
- [ ] `tracey query uncovered` returns 0 for merge requirements

## Phase 2: Actor + Session Proofs

- [ ] Create `verus/actor_spec.rs` — RegistryModel type
- [ ] Prove `r[actor.link.bidirectional]`
- [ ] Prove `r[actor.unlink.bidirectional]`
- [ ] Prove `r[actor.exit.link-cleanup]`
- [ ] Prove `r[actor.exit.monitor-cleanup]`
- [ ] Prove `r[actor.name.unique]`
- [ ] Annotate `crates/clankers-actor/src/registry.rs` with `r[impl actor.*]`
- [ ] Create `verus/session_spec.rs` — TreeModel type
- [ ] Prove `r[session.walk.path-valid]`
- [ ] Prove `r[session.walk.root-anchored]`
- [ ] Prove `r[session.walk.terminates]`
- [ ] Prove `r[session.index.consistent]`
- [ ] Annotate `crates/clankers-session/src/tree/mod.rs` with `r[impl session.*]`
- [ ] Annotate `crates/clankers-session/src/tree/navigation.rs` with `r[impl session.walk.*]`
- [ ] `tracey query uncovered` returns 0 for actor + session requirements

## Phase 3: Protocol Proofs + CI

- [ ] Create `verus/protocol_spec.rs` — frame model
- [ ] Prove `r[protocol.frame.roundtrip]` (modulo serde axiom)
- [ ] Prove `r[protocol.frame.size-reject-write]`
- [ ] Prove `r[protocol.frame.size-reject-read]`
- [ ] Prove `r[protocol.frame.length-encoding]`
- [ ] Annotate `crates/clankers-protocol/src/frame.rs` with `r[impl protocol.*]`
- [ ] Add existing frame tests as `r[verify protocol.*]`
- [ ] Create `scripts/verify.sh` — runs verus + tracey
- [ ] Add `checks.verus-proofs` to flake.nix
- [ ] Add `checks.tracey-coverage` to flake.nix
- [ ] `tracey query uncovered` returns 0 for all requirements
- [ ] `tracey query untested` returns 0 for all requirements
- [ ] `verus --crate-type=lib verus/lib.rs` passes
