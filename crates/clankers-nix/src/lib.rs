//! # clankers-nix
//!
//! Typed Nix operations via snix crates. Replaces fragile string parsing
//! with structured store paths, derivation reading, and flake ref validation.
//!
//! ## Modules
//!
//! - [`store_path`] — Parse nix store paths from build output
//! - [`flakeref`] — Validate flake references before CLI dispatch
//! - [`derivation`] — Read `.drv` files for build metadata
//! - [`refscan`] — Scan text for store path references
//! - [`error`] — Error types
//!
//! ## Phase 1 (nix-compat parsing)
//!
//! Uses `nix-compat` for typed parsing of store paths, derivations, and
//! flake references. No runtime dependencies — pure parsing only.
//!
//! ## Phase 2 (in-process eval, behind `eval` feature)
//!
//! Adds `snix-eval` for evaluating Nix expressions without spawning
//! `nix eval`. Not yet implemented.
//!
//! ## Phase 3 (refscan acceleration, behind `refscan` feature)
//!
//! Adds Wu-Manber scanning via `snix-castore` for large outputs.
//! The regex-based scanner in [`refscan`] works without this feature.

pub mod derivation;
pub mod error;
#[cfg(feature = "eval")]
pub mod eval;
pub mod flakeref;
pub mod refscan;
pub mod store_path;

// Re-exports for convenience
pub use derivation::DerivationInfo;
pub use derivation::InputDrvInfo;
pub use derivation::OutputInfo;
pub use derivation::dependency_summary;
pub use derivation::read_derivation;
pub use error::NixError;
#[cfg(feature = "eval")]
pub use eval::EvalResult;
#[cfg(feature = "eval")]
pub use eval::FlakeOutputs;
#[cfg(feature = "eval")]
pub use eval::evaluate;
#[cfg(feature = "eval")]
pub use eval::evaluate_file;
#[cfg(feature = "eval")]
pub use eval::evaluate_with_timeout;
#[cfg(feature = "eval")]
pub use eval::introspect_flake;
pub use flakeref::FlakeInfo;
pub use flakeref::FlakeSourceType;
pub use flakeref::ParsedFlakeRef;
pub use flakeref::detect_flake;
pub use flakeref::looks_like_flake_ref;
pub use flakeref::parse_flake_ref;
pub use refscan::annotate_store_refs;
pub use refscan::scan_store_refs;
pub use store_path::NixPath;
pub use store_path::extract_store_paths;
pub use store_path::parse_store_path;
