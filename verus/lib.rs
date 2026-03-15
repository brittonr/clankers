//! Verus verification module for clankers core invariants.
//!
//! This crate contains spec fns (formal definitions) and proof fns
//! (machine-checked evidence) for the invariants documented in
//! `docs/requirements.md`.
//!
//! The runtime implementations live in `crates/` — this module models
//! the same logic using vstd's mathematical types (Map, Set, Seq)
//! and proves properties that hold for all inputs, not just test cases.

#[allow(unused_imports)]
use vstd::prelude::*;

mod merge_spec;
mod actor_spec;
mod session_spec;
mod protocol_spec;

verus! {} // ensure the crate compiles as a verus crate
