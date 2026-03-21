//! Error types for clankers-tts.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("voice not found: {name}")]
    VoiceNotFound { name: String },

    #[error("no provider available for voice: {voice}")]
    NoProvider { voice: String },

    #[error("provider error: {message}")]
    Provider { message: String },

    #[error("audio error: {0}")]
    Audio(#[from] hound::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
