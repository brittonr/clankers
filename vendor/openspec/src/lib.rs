//! Spec-driven development (OpenSpec)

pub mod core;

#[cfg(feature = "fs")]
pub mod config;

#[cfg(feature = "fs")]
pub mod engine;

// Re-export core types at the top level
pub use core::*;

#[cfg(feature = "fs")]
pub use config::SpecConfig;
// Re-export SpecEngine and config behind fs feature
#[cfg(feature = "fs")]
pub use engine::SpecEngine;
