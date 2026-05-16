//! Content-addressed artifact identity types.
//!
//! This crate intentionally starts with the small pure core needed by
//! provenance/replay code: stable artifact kinds, canonical envelope versions,
//! redaction classes, and validated hash identifiers. Canonicalization and
//! storage are layered on top of these types by later OpenSpec tasks.

use std::fmt;
use std::str::FromStr;

use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

const BLAKE3_HEX_LEN: usize = 64;
const ARTIFACT_HASH_PREFIX: &str = "b3";

/// Semantic class for a content-addressed Clankers runtime artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactKind {
    /// Prompt text after prompt-assembly normalization.
    Prompt,
    /// Public tool schema/descriptor visible to a model or host.
    ToolDescriptor,
    /// Provider-ready model request envelope after request-shaping policy.
    ModelRequest,
    /// MCP server manifest or tool catalog manifest.
    McpManifest,
    /// Plugin manifest, excluding host-local launch secrets.
    PluginManifest,
    /// Skill reference metadata and safe skill body material.
    SkillReference,
    /// Durable conversation/session block.
    SessionBlock,
}

/// Version of the canonical envelope used to compute artifact identity.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize
)]
#[serde(rename_all = "kebab-case")]
pub enum CanonicalEnvelopeVersion {
    /// Initial Clankers canonical artifact envelope.
    #[default]
    V1,
}

impl CanonicalEnvelopeVersion {
    /// Stable numeric form for compact receipts and future migrations.
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        match self {
            Self::V1 => 1,
        }
    }
}

/// Redaction policy for an artifact envelope or inspect output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RedactionClass {
    /// Safe to render in CLI/TUI/review output.
    Public,
    /// Render summary metadata only; payload may contain local paths or noisy provider details.
    MetadataOnly,
    /// Payload fields must be redacted before display or receipt export.
    RedactedPayload,
    /// Artifact identity may be referenced, but raw payload must not be inspected.
    Secret,
}

impl RedactionClass {
    /// Whether CLI/TUI inspect output may include the artifact payload body.
    #[must_use]
    pub const fn permits_payload_display(self) -> bool {
        matches!(self, Self::Public)
    }
}

/// Validated BLAKE3 content hash for a canonical artifact envelope.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArtifactHash(blake3::Hash);

impl ArtifactHash {
    /// Hash canonical envelope bytes into an artifact hash.
    #[must_use]
    pub fn digest(bytes: &[u8]) -> Self {
        Self(blake3::hash(bytes))
    }

    /// Return the lower-case BLAKE3 hex digest without the `b3:` prefix.
    #[must_use]
    pub fn hex(self) -> String {
        self.0.to_hex().to_string()
    }

    /// Return the stable display/receipt form: `b3:<64-hex>`.
    #[must_use]
    pub fn prefixed(self) -> String {
        format!("{ARTIFACT_HASH_PREFIX}:{}", self.hex())
    }

    /// Construct from a raw BLAKE3 digest.
    #[must_use]
    pub const fn from_blake3(hash: blake3::Hash) -> Self {
        Self(hash)
    }

    /// Expose the underlying BLAKE3 digest for storage backends.
    #[must_use]
    pub const fn into_blake3(self) -> blake3::Hash {
        self.0
    }
}

impl fmt::Debug for ArtifactHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ArtifactHash").field(&self.prefixed()).finish()
    }
}

impl fmt::Display for ArtifactHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.prefixed())
    }
}

impl FromStr for ArtifactHash {
    type Err = ArtifactHashParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let hex = input.strip_prefix("b3:").or_else(|| input.strip_prefix("blake3:")).unwrap_or(input);
        if hex.len() != BLAKE3_HEX_LEN {
            return Err(ArtifactHashParseError::WrongLength { actual: hex.len() });
        }
        let hash = blake3::Hash::from_hex(hex).map_err(|_| ArtifactHashParseError::InvalidHex)?;
        Ok(Self(hash))
    }
}

impl Serialize for ArtifactHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_str(&self.prefixed())
    }
}

impl<'de> Deserialize<'de> for ArtifactHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

/// Error returned when parsing an artifact hash from a receipt or CLI argument.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArtifactHashParseError {
    /// Digest length is not a BLAKE3 hex digest length.
    #[error("artifact hash must contain 64 hex characters, found {actual}")]
    WrongLength {
        /// Number of characters after the optional algorithm prefix.
        actual: usize,
    },
    /// Digest contains a non-hex character or is not a valid BLAKE3 digest.
    #[error("artifact hash must be lower- or upper-case hexadecimal")]
    InvalidHex,
}

/// Header metadata shared by canonical artifact envelopes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactEnvelopeHeader {
    /// Artifact semantic class.
    pub kind: ArtifactKind,
    /// Canonicalization envelope version.
    pub version: CanonicalEnvelopeVersion,
    /// Display/inspect redaction policy.
    pub redaction: RedactionClass,
}

impl ArtifactEnvelopeHeader {
    /// Build a V1 envelope header.
    #[must_use]
    pub const fn v1(kind: ArtifactKind, redaction: RedactionClass) -> Self {
        Self {
            kind,
            version: CanonicalEnvelopeVersion::V1,
            redaction,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_hash_round_trips_prefixed_display_form() {
        let hash = ArtifactHash::digest(b"canonical envelope");
        let rendered = hash.to_string();
        assert!(rendered.starts_with("b3:"));
        assert_eq!(rendered.len(), 67);
        assert_eq!(rendered.parse::<ArtifactHash>(), Ok(hash));
    }

    #[test]
    fn artifact_hash_accepts_legacy_blake3_prefix_and_raw_hex() {
        let hash = ArtifactHash::digest(b"same digest");
        assert_eq!(hash.hex().parse::<ArtifactHash>(), Ok(hash));
        assert_eq!(format!("blake3:{}", hash.hex()).parse::<ArtifactHash>(), Ok(hash));
    }

    #[test]
    fn artifact_hash_rejects_short_or_non_hex_values() {
        assert_eq!("b3:abc".parse::<ArtifactHash>(), Err(ArtifactHashParseError::WrongLength { actual: 3 }));
        let bad = format!("b3:{}z", "0".repeat(63));
        assert_eq!(bad.parse::<ArtifactHash>(), Err(ArtifactHashParseError::InvalidHex));
    }

    #[test]
    fn artifact_hash_serde_uses_prefixed_string() {
        let hash = ArtifactHash::digest(b"json receipt");
        let json = serde_json::to_string(&hash).expect("serialize artifact hash");
        assert_eq!(json, format!("\"{hash}\""));
        assert_eq!(serde_json::from_str::<ArtifactHash>(&json).ok(), Some(hash));
    }

    #[test]
    fn envelope_header_serializes_stable_kebab_case_metadata() {
        let header = ArtifactEnvelopeHeader::v1(ArtifactKind::ToolDescriptor, RedactionClass::RedactedPayload);
        let json = serde_json::to_value(&header).expect("serialize envelope header");
        assert_eq!(json["kind"], "tool-descriptor");
        assert_eq!(json["version"], "v1");
        assert_eq!(json["redaction"], "redacted-payload");
    }

    #[test]
    fn only_public_redaction_displays_payloads() {
        assert!(RedactionClass::Public.permits_payload_display());
        assert!(!RedactionClass::MetadataOnly.permits_payload_display());
        assert!(!RedactionClass::RedactedPayload.permits_payload_display());
        assert!(!RedactionClass::Secret.permits_payload_display());
    }
}
