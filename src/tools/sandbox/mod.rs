//! Process sandbox for agent tool execution
//!
//! Two concerns, cleanly separated:
//!
//! 1. **Path policy** — which filesystem paths any tool may access. Enforced once in the tool
//!    dispatch layer (`turn.rs`), not per-tool.
//!
//! 2. **Bash child sandbox** — environment scrubbing and optional kernel-level restrictions applied
//!    to spawned shell commands. This is where the real attack surface lives.

mod landlock;
mod policy;

// Re-export public API
pub use landlock::apply_landlock_to_current;
pub use policy::{check_path, init_policy, sanitized_env, PathPolicy};
