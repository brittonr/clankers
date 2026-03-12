//! Lightweight native actor primitives for agent process trees.
//!
//! Tokio tasks with Erlang-style signals, linking, and supervision.
//! No WASM dependency — actors are native tokio tasks.

pub mod process;
pub mod registry;
pub mod signal;
pub mod supervisor;

pub use process::DeathReason;
pub use process::ProcessHandle;
pub use process::ProcessId;
pub use registry::ProcessInfo;
pub use registry::ProcessRegistry;
pub use signal::Signal;
pub use supervisor::Supervisor;
pub use supervisor::SupervisorConfig;
pub use supervisor::SupervisorStrategy;
