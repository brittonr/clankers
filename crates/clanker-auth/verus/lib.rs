//! Verus verification module for clanker-auth framework invariants.
//!
//! Proves properties of the generic token builder, verifier, and credential
//! logic that hold for any `Cap` implementation. Capabilities are modeled
//! abstractly as natural numbers with a containment relation and delegate
//! predicate supplied as ghost parameters.
//!
//! These proofs compose with downstream domain-specific proofs (e.g.,
//! clankers' pattern matching and file access containment) to give
//! end-to-end authorization correctness.

#[allow(unused_imports)]
use vstd::prelude::*;

mod auth_spec;

verus! {} // ensure the crate compiles as a verus crate
