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
pub mod flakeref;
pub mod refscan;
pub mod store_path;

// Re-exports for convenience
pub use derivation::{DerivationInfo, InputDrvInfo, OutputInfo, dependency_summary, read_derivation};
pub use error::NixError;
pub use flakeref::{
    FlakeInfo, FlakeSourceType, ParsedFlakeRef, detect_flake, looks_like_flake_ref, parse_flake_ref,
};
pub use refscan::{annotate_store_refs, scan_store_refs};
pub use store_path::{NixPath, extract_store_paths, parse_store_path};
