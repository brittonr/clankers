//! Process sandbox for agent tool execution
//!
//! Two concerns, cleanly separated:
//!
//! 1. **Path policy** — which filesystem paths any tool may access. Enforced once in the tool
//!    dispatch layer (`turn.rs`), not per-tool. Canonical definitions in `clankers-util`.
//!
//! 2. **Bash child sandbox** — environment scrubbing and optional kernel-level restrictions applied
//!    to spawned shell commands. This is where the real attack surface lives.

mod landlock;
mod policy;

// Re-export public API
// Path policy — canonical definitions in clankers-util
pub use clankers_util::path_policy::PathPolicy;
pub use clankers_util::path_policy::check_path;
pub use clankers_util::path_policy::init_policy;
pub use landlock::apply_landlock_to_current;
// Environment sanitization — stays local
pub use policy::sanitized_env;
