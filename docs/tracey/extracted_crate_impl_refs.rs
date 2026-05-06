//! Tracey implementation anchors for invariants implemented in extracted crates.
//!
//! The runtime implementations for these requirements live in direct Cargo git
//! dependencies (`graggle` and `clanker-actor`) rather than workspace-local source.
//! Tracey only scans this repository, so this file keeps the formal requirements
//! connected to the production implementation boundary while Verus proof refs stay
//! in `verus/merge_spec.rs` and `verus/actor_spec.rs`.

// graggle runtime invariants.
// r[impl merge.dag.sentinels]
// r[impl merge.dag.reachability]
// r[impl merge.dag.acyclicity]
// r[impl merge.insert.preserves-dag]
// r[impl merge.order-independence]
// r[impl merge.from-text.linear]
// r[impl merge.delete.ghost]

// clanker-actor runtime invariants.
// r[impl actor.link.bidirectional]
// r[impl actor.unlink.bidirectional]
// r[impl actor.exit.link-cleanup]
// r[impl actor.exit.monitor-cleanup]
// r[impl actor.name.unique]
