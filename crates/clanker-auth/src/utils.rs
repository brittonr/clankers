//! Utility functions for capability-based authorization.
//!
//! Provides safe time access without panics.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::time::SystemTime;
use std::time::UNIX_EPOCH;

/// Get current Unix timestamp in seconds.
///
/// Returns 0 if system time is before UNIX epoch (should never happen
/// on properly configured systems, but prevents panics).
///
/// # Tiger Style
///
/// - No `.expect()` or `.unwrap()` - safe fallback to 0
/// - Inline for hot path performance
#[inline]
pub fn current_time_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
