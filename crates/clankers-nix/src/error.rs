//! Error types for clankers-nix.

use snafu::prelude::*;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
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
}
