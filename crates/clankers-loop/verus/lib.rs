//! Verus formal verification specs for clankers-loop.
//!
//! These specs verify the core invariants of the loop engine:
//!
//! 1. **Bounded iteration**: `record_iteration` always terminates
//!    within `max_iterations` steps.
//!
//! 2. **State machine correctness**: transitions follow
//!    Pending → Running → {Completed, Stopped, Failed}.
//!    No backward transitions.
//!
//! 3. **Break condition purity**: `BreakCondition::check` is
//!    deterministic — same inputs always produce the same output.
//!
//! To verify: `verus verus/lib.rs`
//!
//! These are spec-only files. The production code lives in `src/`.

mod loop_state_spec;
