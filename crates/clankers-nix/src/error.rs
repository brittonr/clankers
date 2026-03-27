//! Error types for clankers-nix.

use snafu::prelude::*;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
#[cfg_attr(dylint_lib = "tigerstyle", allow(acronym_style, reason = "NotAStorePath uses English article 'A', not an acronym"))]
pub enum NixError {
    /// The input string is not a valid nix store path.
    #[snafu(display("not a store path: {path}"))]
    NotAStorePath { path: String },

    /// Failed to parse a store path's hash or name.
    #[snafu(display("invalid store path '{path}': {reason}"))]
    InvalidStorePath { path: String, reason: String },

    /// The input string is not a valid flake reference.
    #[snafu(display("invalid flake reference '{input}': {reason}"))]
    InvalidFlakeRef { input: String, reason: String },

    /// Failed to read a .drv file from disk.
    #[snafu(display("failed to read derivation '{path}': {source}"))]
    DerivationIo {
        path: String,
        source: std::io::Error,
    },

    /// The .drv file contents are malformed.
    #[snafu(display("failed to parse derivation '{path}': {reason}"))]
    DerivationParse { path: String, reason: String },

    /// Nix expression evaluation failed.
    #[snafu(display("eval failed for '{expr}': {reason}"))]
    EvalFailed {
        expr: String,
        reason: String,
        /// Whether the failure was due to impure operations (import, IO, etc.)
        is_impure: bool,
    },

    /// Evaluation exceeded the time limit.
    #[snafu(display("eval timed out after {seconds}s"))]
    EvalTimeout { seconds: u64 },

    /// Serialized evaluation output exceeded the size limit.
    #[snafu(display("eval output too large: {size} bytes (max {max})"))]
    EvalOutputTooLarge { size: usize, max: usize },
}
