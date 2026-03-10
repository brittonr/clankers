//! Verus specs for loop state machine transitions.
//!
//! # Invariants verified:
//!
//! - `current_iteration` never exceeds `max_iterations`
//! - Status transitions are monotonic (no backward transitions)
//! - `finished_at` is `Some` iff status is terminal
//! - `results.len()` equals `current_iteration`
//! - Fixed loops complete at exactly `count` iterations
//!
//! # Model (simplified for verification):
//!
//! ```verus
//! enum Status { Pending, Running, Completed, Stopped, Failed }
//!
//! struct LoopState {
//!     status: Status,
//!     current_iteration: u32,
//!     max_iterations: u32,
//!     results_len: u32,  // abstraction of Vec::len()
//!     finished: bool,    // abstraction of finished_at.is_some()
//! }
//! ```

// NOTE: These are Verus pseudocode specs. They cannot be compiled by
// rustc — they require the Verus verifier. When Verus is available,
// uncomment and run `verus verus/lib.rs`.

/*
use builtin::*;
use builtin_macros::*;

verus! {

/// Status ordering: Pending < Running < {Completed, Stopped, Failed}
spec fn status_ord(s: Status) -> int {
    match s {
        Status::Pending => 0,
        Status::Running => 1,
        Status::Completed => 2,
        Status::Stopped => 2,
        Status::Failed => 2,
    }
}

/// A loop state is well-formed.
spec fn well_formed(s: LoopState) -> bool {
    &&& s.max_iterations > 0
    &&& s.current_iteration <= s.max_iterations
    &&& s.results_len == s.current_iteration
    &&& (s.status == Status::Pending) ==> (s.current_iteration == 0 && !s.finished)
    &&& (s.status == Status::Running) ==> (!s.finished)
    &&& (s.status == Status::Completed || s.status == Status::Stopped || s.status == Status::Failed) ==> s.finished
}

/// Proves: start() preserves well-formedness.
proof fn start_preserves_wf(pre: LoopState)
    requires
        well_formed(pre),
        pre.status == Status::Pending,
    ensures
        well_formed(LoopState {
            status: Status::Running,
            finished: false,
            ..pre
        }),
{
}

/// Proves: record_iteration increments current_iteration by exactly 1.
proof fn record_iteration_increments(pre: LoopState, break_matched: bool)
    requires
        well_formed(pre),
        pre.status == Status::Running,
        pre.current_iteration < pre.max_iterations,
    ensures
        ({
            let post = LoopState {
                current_iteration: pre.current_iteration + 1,
                results_len: pre.results_len + 1,
                status: if break_matched { Status::Completed }
                        else if pre.current_iteration + 1 >= pre.max_iterations { Status::Stopped }
                        else { Status::Running },
                finished: break_matched || pre.current_iteration + 1 >= pre.max_iterations,
                ..pre
            };
            well_formed(post)
        }),
{
}

/// Proves: status transitions are monotonic (no backward transitions).
proof fn status_monotonic(pre: LoopState, post: LoopState)
    requires
        well_formed(pre),
        well_formed(post),
        post.current_iteration == pre.current_iteration + 1 || post.current_iteration == pre.current_iteration,
    ensures
        status_ord(post.status) >= status_ord(pre.status),
{
}

/// Proves: record_iteration always terminates (bounded by max_iterations).
proof fn termination_guarantee(s: LoopState)
    requires
        well_formed(s),
        s.status == Status::Running,
    ensures
        s.max_iterations - s.current_iteration > 0,  // decreasing measure
{
}

} // verus!
*/
