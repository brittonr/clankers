//! Iterative loop execution engine for clankers.
//!
//! Provides three loop kinds that an agent can use for repetitive work:
//!
//! - **Fixed** — run N iterations, collect results.
//! - **Until** — run until a condition matches output (regex or substring).
//! - **Poll** — run at intervals until a condition matches or timeout.
//!
//! Loops are managed by a `LoopEngine` that tracks active loops and emits
//! events on each iteration. The actual work (running a prompt, executing
//! a command) is done by a callback — the engine only handles iteration,
//! condition checking, and state tracking.

pub mod condition;
pub mod engine;
pub mod iteration;
pub mod truncation;

pub use truncation::OutputTruncationConfig;
pub use truncation::TruncationResult;
pub use truncation::cleanup_temp_files;
pub use truncation::truncate_tool_output;

pub use condition::BreakCondition;
pub use condition::parse_break_condition;
pub use engine::LoopEngine;
pub use engine::LoopEvent;
pub use iteration::LoopDef;
pub use iteration::LoopId;
pub use iteration::LoopKind;
pub use iteration::LoopState;
pub use iteration::LoopStatus;
pub use iteration::MAX_ACTIVE_LOOPS;
pub use iteration::MAX_ITERATIONS_HARD_LIMIT;
