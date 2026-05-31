//! Content-addressed artifact identity and canonicalization types.
//!
//! This crate intentionally starts with the small pure core needed by
//! provenance/replay code: stable artifact kinds, canonical envelope versions,
//! redaction classes, validated hash identifiers, and deterministic canonical
//! envelopes. Immutable storage and receipt plumbing are layered on top of these
//! types by later Cairn tasks.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use thiserror::Error;

const BLAKE3_HEX_LEN: usize = 64;
const ARTIFACT_HASH_PREFIX: &str = "b3";
const VOLATILE_FIELD_NAMES: &[&str] = &[
    "timestamp",
    "created_at",
    "updated_at",
    "started_at",
    "completed_at",
    "duration_ms",
    "display_name",
    "label",
];
const SECRET_FIELD_NAMES: &[&str] = &[
    "authorization",
    "api_key",
    "api-key",
    "access_token",
    "refresh_token",
    "password",
    "secret",
    "token",
];
const HOST_LOCAL_FIELD_NAMES: &[&str] = &["path", "cwd", "workdir", "home", "socket_path"];

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

impl ArtifactKind {
    /// Stable kind string used by golden fixtures and external receipts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prompt => "prompt",
            Self::ToolDescriptor => "tool-descriptor",
            Self::ModelRequest => "model-request",
            Self::McpManifest => "mcp-manifest",
            Self::PluginManifest => "plugin-manifest",
            Self::SkillReference => "skill-reference",
            Self::SessionBlock => "session-block",
        }
    }
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

/// Canonical V1 artifact envelope used as the stable hash preimage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalArtifactEnvelope {
    /// Header fields that define artifact interpretation.
    pub header: ArtifactEnvelopeHeader,
    /// Normalized semantic payload.
    pub payload: Value,
    /// Content hashes this artifact depends on, sorted and deduplicated.
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<ArtifactHash>,
}

impl CanonicalArtifactEnvelope {
    /// Build a canonical envelope from an arbitrary JSON payload.
    pub fn new(
        kind: ArtifactKind,
        redaction: RedactionClass,
        payload: Value,
        dependencies: impl IntoIterator<Item = ArtifactHash>,
    ) -> Result<Self, CanonicalizationError> {
        let normalized_payload = normalize_payload(payload)?;
        let mut dependencies = dependencies.into_iter().collect::<Vec<_>>();
        dependencies.sort_by_key(|hash| hash.hex());
        dependencies.dedup();
        Ok(Self {
            header: ArtifactEnvelopeHeader::v1(kind, redaction),
            payload: normalized_payload,
            dependencies,
        })
    }

    /// Return deterministic JSON bytes for this envelope.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CanonicalizationError> {
        serde_json::to_vec(self).map_err(CanonicalizationError::Serialize)
    }

    /// Hash the canonical envelope bytes.
    pub fn hash(&self) -> Result<ArtifactHash, CanonicalizationError> {
        Ok(ArtifactHash::digest(&self.canonical_bytes()?))
    }
}

/// Canonicalize and hash one supported artifact payload.
pub fn canonicalize_artifact(
    kind: ArtifactKind,
    redaction: RedactionClass,
    payload: Value,
    dependencies: impl IntoIterator<Item = ArtifactHash>,
) -> Result<(CanonicalArtifactEnvelope, ArtifactHash), CanonicalizationError> {
    let envelope = CanonicalArtifactEnvelope::new(kind, redaction, payload, dependencies)?;
    let hash = envelope.hash()?;
    Ok((envelope, hash))
}

/// On-disk immutable artifact store rooted under a Clankers state directory.
#[derive(Debug, Clone)]
pub struct ArtifactStore {
    root: PathBuf,
}

impl ArtifactStore {
    /// Create a store handle rooted at `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Store an envelope by its content hash and return that hash.
    pub fn put(&self, envelope: &CanonicalArtifactEnvelope) -> Result<ArtifactHash, ArtifactStoreError> {
        let hash = envelope.hash()?;
        let record = StoredArtifactRecord::new(hash, envelope.clone());
        let bytes = serde_json::to_vec_pretty(&record).map_err(CanonicalizationError::Serialize)?;
        let path = self.artifact_path(hash);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(ArtifactStoreError::Io)?;
        }
        match fs::read(&path) {
            Ok(existing) if existing == bytes => Ok(hash),
            Ok(_) => Err(ArtifactStoreError::ImmutableCollision { hash }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::write(path, bytes).map_err(ArtifactStoreError::Io)?;
                Ok(hash)
            }
            Err(error) => Err(ArtifactStoreError::Io(error)),
        }
    }

    /// Load an immutable artifact by hash.
    pub fn get(&self, hash: ArtifactHash) -> Result<CanonicalArtifactEnvelope, ArtifactStoreError> {
        let path = self.artifact_path(hash);
        let bytes = fs::read(path).map_err(|error| map_not_found(error, hash))?;
        let record: StoredArtifactRecord = serde_json::from_slice(&bytes).map_err(ArtifactStoreError::Decode)?;
        if record.hash != hash {
            return Err(ArtifactStoreError::HashMismatch {
                requested: hash,
                found: record.hash,
            });
        }
        if record.envelope.hash()? != hash {
            return Err(ArtifactStoreError::PayloadHashMismatch { hash });
        }
        Ok(record.envelope)
    }

    /// Inspect an artifact without exposing redacted payload bodies.
    pub fn inspect(&self, hash: ArtifactHash) -> Result<ArtifactInspectSummary, ArtifactStoreError> {
        let envelope = self.get(hash)?;
        Ok(ArtifactInspectSummary::from_envelope(hash, &envelope))
    }

    /// Update a mutable human-readable pointer to an immutable artifact hash.
    pub fn link_name(&self, kind: ArtifactKind, name: &str, hash: ArtifactHash) -> Result<(), ArtifactStoreError> {
        validate_name(name)?;
        self.get(hash)?;
        let pointer = NamePointer {
            kind,
            name: name.to_owned(),
            hash,
        };
        let path = self.name_path(kind, name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(ArtifactStoreError::Io)?;
        }
        let bytes = serde_json::to_vec_pretty(&pointer).map_err(ArtifactStoreError::Encode)?;
        fs::write(path, bytes).map_err(ArtifactStoreError::Io)
    }

    /// Resolve a mutable human-readable pointer to the current artifact hash.
    pub fn resolve_name(&self, kind: ArtifactKind, name: &str) -> Result<ArtifactHash, ArtifactStoreError> {
        validate_name(name)?;
        let path = self.name_path(kind, name);
        let bytes = fs::read(path).map_err(ArtifactStoreError::Io)?;
        let pointer: NamePointer = serde_json::from_slice(&bytes).map_err(ArtifactStoreError::Decode)?;
        if pointer.kind != kind || pointer.name != name {
            return Err(ArtifactStoreError::NamePointerMismatch {
                kind,
                name: name.to_owned(),
            });
        }
        Ok(pointer.hash)
    }

    fn artifact_path(&self, hash: ArtifactHash) -> PathBuf {
        let hex = hash.hex();
        self.root.join("artifacts").join("b3").join(&hex[..2]).join(format!("{hex}.json"))
    }

    fn name_path(&self, kind: ArtifactKind, name: &str) -> PathBuf {
        self.root.join("names").join(kind.as_str()).join(format!("{name}.json"))
    }
}

/// Stored immutable artifact record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredArtifactRecord {
    /// Hash key under which this record is stored.
    pub hash: ArtifactHash,
    /// Canonical envelope whose bytes produce `hash`.
    pub envelope: CanonicalArtifactEnvelope,
}

impl StoredArtifactRecord {
    /// Build a stored artifact record.
    #[must_use]
    pub const fn new(hash: ArtifactHash, envelope: CanonicalArtifactEnvelope) -> Self {
        Self { hash, envelope }
    }
}

/// Mutable metadata pointer from a human-readable name to an immutable hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamePointer {
    /// Artifact kind namespace for this name.
    pub kind: ArtifactKind,
    /// Human-readable pointer name.
    pub name: String,
    /// Current immutable artifact target.
    pub hash: ArtifactHash,
}

/// Role an artifact hash plays in an execution or review receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReceiptArtifactRole {
    /// Provider-ready model request artifact.
    ModelRequest,
    /// Tool descriptor or invocation descriptor artifact.
    ToolDescriptor,
    /// Durable session/conversation block artifact.
    SessionBlock,
    /// Review evidence, prompt, or policy artifact.
    ReviewArtifact,
}

/// Hash reference embedded in model, tool, session, or review receipts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptArtifactRef {
    /// Receipt role for this artifact.
    pub role: ReceiptArtifactRole,
    /// Artifact kind stored at `hash`.
    pub kind: ArtifactKind,
    /// Immutable content hash.
    pub hash: ArtifactHash,
    /// Inspect redaction policy copied from the canonical envelope.
    pub redaction: RedactionClass,
}

impl ReceiptArtifactRef {
    /// Create a receipt reference from a stored envelope and role.
    #[must_use]
    pub fn from_envelope(role: ReceiptArtifactRole, hash: ArtifactHash, envelope: &CanonicalArtifactEnvelope) -> Self {
        Self {
            role,
            kind: envelope.header.kind,
            hash,
            redaction: envelope.header.redaction,
        }
    }
}

/// Collection of artifact hashes that influenced an execution receipt.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptArtifacts {
    /// Model/tool/session/review artifact references.
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ReceiptArtifactRef>,
}

impl ReceiptArtifacts {
    /// Add a receipt artifact reference.
    pub fn push(&mut self, reference: ReceiptArtifactRef) {
        self.artifacts.push(reference);
    }

    /// Return all hashes in deterministic role/kind/hash order.
    #[must_use]
    pub fn sorted(mut self) -> Self {
        self.artifacts.sort_by_key(|reference| (reference.role, reference.kind, reference.hash.hex()));
        self
    }
}

/// Safe inspect output for CLI/TUI/review display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactInspectSummary {
    /// Inspected hash.
    pub hash: ArtifactHash,
    /// Artifact kind.
    pub kind: ArtifactKind,
    /// Canonical envelope version.
    pub version: CanonicalEnvelopeVersion,
    /// Redaction policy applied to inspect output.
    pub redaction: RedactionClass,
    /// Dependency hashes referenced by this artifact.
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<ArtifactHash>,
    /// Payload only when redaction policy permits safe display.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    /// Human-readable omission reason for redacted payloads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redacted_reason: Option<String>,
}

impl ArtifactInspectSummary {
    /// Build safe inspect output from an envelope.
    #[must_use]
    pub fn from_envelope(hash: ArtifactHash, envelope: &CanonicalArtifactEnvelope) -> Self {
        let payload = envelope.header.redaction.permits_payload_display().then(|| envelope.payload.clone());
        let redacted_reason = (!envelope.header.redaction.permits_payload_display())
            .then(|| format!("payload hidden by {:?} redaction policy", envelope.header.redaction));
        Self {
            hash,
            kind: envelope.header.kind,
            version: envelope.header.version,
            redaction: envelope.header.redaction,
            dependencies: envelope.dependencies.clone(),
            payload,
            redacted_reason,
        }
    }
}

/// Cache key for an opt-in deterministic pure result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PureCacheKey(ArtifactHash);

impl PureCacheKey {
    /// Return the display/receipt hash for this cache key.
    #[must_use]
    pub fn hash(self) -> ArtifactHash {
        self.0
    }
}

/// Explicit declaration of all deterministic inputs that influence a pure result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterministicInputDeclaration {
    /// Artifact inputs consumed by the operation.
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub artifact_hashes: Vec<ArtifactHash>,
    /// File content hashes consumed by the operation.
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub file_input_hashes: Vec<ArtifactHash>,
    /// Environment variable names explicitly admitted as deterministic inputs.
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub env_allowlist: Vec<String>,
    /// Tool or operation version string.
    pub tool_version: String,
    /// Declared no-hidden-effect profile.
    pub effect_profile: EffectProfile,
}

impl DeterministicInputDeclaration {
    /// Normalize order-insensitive fields before cache-key hashing.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.artifact_hashes.sort_by_key(|hash| hash.hex());
        self.artifact_hashes.dedup();
        self.file_input_hashes.sort_by_key(|hash| hash.hex());
        self.file_input_hashes.dedup();
        self.env_allowlist.sort();
        self.env_allowlist.dedup();
        self
    }

    /// Compute the deterministic pure-result cache key.
    pub fn cache_key(self) -> Result<PureCacheKey, CanonicalizationError> {
        let bytes = serde_json::to_vec(&self.normalized()).map_err(CanonicalizationError::Serialize)?;
        Ok(PureCacheKey(ArtifactHash::digest(&bytes)))
    }
}

/// Declared side-effect profile for cache eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EffectProfile {
    /// No hidden reads, writes, network, process, clock, or undeclared env access.
    NoHiddenEffects,
}

/// Side-effect class that can deny pure cache eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EffectClass {
    /// Reads an environment variable outside the declaration allowlist.
    UndeclaredEnvironment,
    /// Uses wall-clock time or timers as semantic input.
    Time,
    /// Uses network access.
    Network,
    /// Starts a shell or process.
    Process,
    /// Mutates filesystem state.
    FileMutation,
}

/// Cache eligibility decision for a pure-result operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheEligibility {
    /// Computed key when eligible.
    pub key: Option<PureCacheKey>,
    /// Denied effect classes, empty when eligible.
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub denied_effects: Vec<EffectClass>,
}

impl CacheEligibility {
    /// Evaluate cache eligibility from explicit declaration and observed effects.
    pub fn evaluate(
        declaration: DeterministicInputDeclaration,
        observed_effects: impl IntoIterator<Item = EffectClass>,
    ) -> Result<Self, CanonicalizationError> {
        let mut denied_effects = observed_effects.into_iter().collect::<Vec<_>>();
        denied_effects.sort();
        denied_effects.dedup();
        if denied_effects.is_empty() {
            Ok(Self {
                key: Some(declaration.cache_key()?),
                denied_effects,
            })
        } else {
            Ok(Self {
                key: None,
                denied_effects,
            })
        }
    }

    /// Whether this operation may use the pure-result cache.
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        self.key.is_some() && self.denied_effects.is_empty()
    }
}

/// Receipt emitted for a pure-result cache lookup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PureCacheReceipt {
    /// Cache key when eligible.
    pub key: Option<PureCacheKey>,
    /// Whether an existing result was reused.
    pub hit: bool,
    /// Denied effects when cache use was blocked.
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub denied_effects: Vec<EffectClass>,
}

/// File-backed pure-result cache storing JSON values by deterministic key.
#[derive(Debug, Clone)]
pub struct PureResultCache {
    root: PathBuf,
}

impl PureResultCache {
    /// Create a pure-result cache under `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Load a cached result if eligible and present.
    pub fn get(&self, eligibility: &CacheEligibility) -> Result<(Option<Value>, PureCacheReceipt), ArtifactStoreError> {
        let Some(key) = eligibility.key else {
            return Ok((None, PureCacheReceipt {
                key: None,
                hit: false,
                denied_effects: eligibility.denied_effects.clone(),
            }));
        };
        let path = self.cache_path(key);
        match fs::read(path) {
            Ok(bytes) => {
                let value = serde_json::from_slice(&bytes).map_err(ArtifactStoreError::Decode)?;
                Ok((Some(value), PureCacheReceipt {
                    key: Some(key),
                    hit: true,
                    denied_effects: Vec::new(),
                }))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok((None, PureCacheReceipt {
                key: Some(key),
                hit: false,
                denied_effects: Vec::new(),
            })),
            Err(error) => Err(ArtifactStoreError::Io(error)),
        }
    }

    /// Store an eligible pure result by cache key.
    pub fn put(&self, key: PureCacheKey, value: &Value) -> Result<(), ArtifactStoreError> {
        let path = self.cache_path(key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(ArtifactStoreError::Io)?;
        }
        let bytes = serde_json::to_vec_pretty(value).map_err(ArtifactStoreError::Encode)?;
        fs::write(path, bytes).map_err(ArtifactStoreError::Io)
    }

    fn cache_path(&self, key: PureCacheKey) -> PathBuf {
        let hex = key.hash().hex();
        self.root.join("pure-results").join("b3").join(&hex[..2]).join(format!("{hex}.json"))
    }
}

/// Artifact store failures.
#[derive(Debug, Error)]
pub enum ArtifactStoreError {
    /// Canonicalization failed while hashing or serializing an envelope.
    #[error(transparent)]
    Canonicalization(#[from] CanonicalizationError),
    /// Filesystem operation failed.
    #[error("artifact store I/O failed: {0}")]
    Io(io::Error),
    /// Stored JSON could not be decoded.
    #[error("artifact store record could not be decoded: {0}")]
    Decode(serde_json::Error),
    /// Name pointer JSON could not be encoded.
    #[error("artifact name pointer could not be encoded: {0}")]
    Encode(serde_json::Error),
    /// The requested artifact is absent from the immutable store.
    #[error("artifact {hash} is missing")]
    Missing { hash: ArtifactHash },
    /// An existing immutable artifact path contains bytes for different content.
    #[error("artifact store collision for immutable hash {hash}")]
    ImmutableCollision { hash: ArtifactHash },
    /// Stored record hash does not match the requested path hash.
    #[error("stored artifact hash mismatch: requested {requested}, found {found}")]
    HashMismatch {
        requested: ArtifactHash,
        found: ArtifactHash,
    },
    /// Stored envelope no longer hashes to its record hash.
    #[error("stored artifact payload does not hash to {hash}")]
    PayloadHashMismatch { hash: ArtifactHash },
    /// Name contains path separators or is empty.
    #[error("artifact name `{name}` is not safe metadata pointer name")]
    UnsafeName { name: String },
    /// Pointer file did not match the requested namespace/name.
    #[error("artifact name pointer did not match requested {kind:?}/{name}")]
    NamePointerMismatch { kind: ArtifactKind, name: String },
}

fn map_not_found(error: io::Error, hash: ArtifactHash) -> ArtifactStoreError {
    if error.kind() == io::ErrorKind::NotFound {
        ArtifactStoreError::Missing { hash }
    } else {
        ArtifactStoreError::Io(error)
    }
}

fn validate_name(name: &str) -> Result<(), ArtifactStoreError> {
    let is_empty_name = name.is_empty();
    let has_forward_slash = name.contains('/');
    let has_backward_slash = name.contains('\\');
    let is_single_component = Path::new(name).components().count() == 1;
    if is_empty_name {
        return Err(ArtifactStoreError::UnsafeName { name: name.to_owned() });
    }
    if has_forward_slash {
        return Err(ArtifactStoreError::UnsafeName { name: name.to_owned() });
    }
    if has_backward_slash {
        return Err(ArtifactStoreError::UnsafeName { name: name.to_owned() });
    }
    if !is_single_component {
        return Err(ArtifactStoreError::UnsafeName { name: name.to_owned() });
    }
    Ok(())
}

/// Canonicalization failures for unsupported or unsafe payloads.
#[derive(Debug, Error)]
pub enum CanonicalizationError {
    /// A payload field contains secret material that must not enter a canonical inspectable
    /// preimage.
    #[error("secret-bearing field `{field}` must be redacted before artifact canonicalization")]
    SecretField { field: String },
    /// JSON serialization failed.
    #[error("failed to serialize canonical artifact envelope: {0}")]
    Serialize(serde_json::Error),
}

fn normalize_payload(value: Value) -> Result<Value, CanonicalizationError> {
    match value {
        Value::Object(object) => normalize_object(object),
        Value::Array(items) => {
            items.into_iter().map(normalize_payload).collect::<Result<Vec<_>, _>>().map(Value::Array)
        }
        scalar => Ok(scalar),
    }
}

fn normalize_object(object: Map<String, Value>) -> Result<Value, CanonicalizationError> {
    let mut normalized = Map::new();
    for (key, value) in object {
        let normalized_key = key.to_ascii_lowercase();
        if VOLATILE_FIELD_NAMES.contains(&normalized_key.as_str())
            || HOST_LOCAL_FIELD_NAMES.contains(&normalized_key.as_str())
        {
            continue;
        }
        if SECRET_FIELD_NAMES.contains(&normalized_key.as_str()) {
            return Err(CanonicalizationError::SecretField { field: key });
        }
        normalized.insert(key, normalize_payload(value)?);
    }
    Ok(Value::Object(normalized))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

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

    #[test]
    fn canonicalization_ignores_map_ordering_and_volatile_display_fields() {
        let left = json!({"name":"answer","display_name":"Pretty","schema":{"b":2,"a":1},"timestamp":"now"});
        let right = json!({"timestamp":"later","schema":{"a":1,"b":2},"name":"answer","display_name":"Other"});
        let (_, left_hash) = canonicalize_artifact(ArtifactKind::ToolDescriptor, RedactionClass::Public, left, [])
            .expect("canonicalize left payload");
        let (_, right_hash) = canonicalize_artifact(ArtifactKind::ToolDescriptor, RedactionClass::Public, right, [])
            .expect("canonicalize right payload");
        assert_eq!(left_hash, right_hash);
    }

    #[test]
    fn canonicalization_changes_hash_for_semantic_payload_changes() {
        let (_, first_hash) = canonicalize_artifact(
            ArtifactKind::Prompt,
            RedactionClass::Public,
            json!({"messages":[{"role":"system","content":"be concise"}]}),
            [],
        )
        .expect("canonicalize prompt");
        let (_, second_hash) = canonicalize_artifact(
            ArtifactKind::Prompt,
            RedactionClass::Public,
            json!({"messages":[{"role":"system","content":"be thorough"}]}),
            [],
        )
        .expect("canonicalize changed prompt");
        assert_ne!(first_hash, second_hash);
    }

    #[test]
    fn canonicalization_rejects_secret_fields_and_excludes_host_local_paths() {
        let error = canonicalize_artifact(
            ArtifactKind::ModelRequest,
            RedactionClass::RedactedPayload,
            json!({"model":"gpt","authorization":"Bearer secret"}),
            [],
        )
        .expect_err("authorization must be rejected");
        assert!(matches!(error, CanonicalizationError::SecretField { .. }));

        let (envelope, _) = canonicalize_artifact(
            ArtifactKind::PluginManifest,
            RedactionClass::MetadataOnly,
            json!({"name":"local-plugin","path":"/tmp/plugin.wasm","commands":["run"]}),
            [],
        )
        .expect("host-local path is excluded");
        assert_eq!(envelope.payload, json!({"commands":["run"],"name":"local-plugin"}));
    }

    #[test]
    fn dependencies_are_sorted_and_deduplicated_in_canonical_envelope() {
        let low = ArtifactHash::digest(b"aaa");
        let high = ArtifactHash::digest(b"zzz");
        let (envelope, _) =
            canonicalize_artifact(ArtifactKind::SessionBlock, RedactionClass::Public, json!({"messages":[]}), [
                high, low, high,
            ])
            .expect("canonicalize session block");
        assert_eq!(envelope.dependencies, vec![low, high]);
    }

    #[test]
    fn artifact_store_preserves_immutable_artifacts_when_name_pointer_moves() {
        let tempdir = tempfile::tempdir().expect("temp artifact store");
        let store = ArtifactStore::new(tempdir.path());
        let (first, first_hash) = canonicalize_artifact(
            ArtifactKind::Prompt,
            RedactionClass::Public,
            json!({"messages":[{"role":"system","content":"first"}]}),
            [],
        )
        .expect("first prompt");
        let (second, second_hash) = canonicalize_artifact(
            ArtifactKind::Prompt,
            RedactionClass::Public,
            json!({"messages":[{"role":"system","content":"second"}]}),
            [],
        )
        .expect("second prompt");

        assert_eq!(store.put(&first).expect("store first"), first_hash);
        assert_eq!(store.put(&second).expect("store second"), second_hash);
        store.link_name(ArtifactKind::Prompt, "system", first_hash).expect("link first");
        assert_eq!(store.resolve_name(ArtifactKind::Prompt, "system").expect("resolve first"), first_hash);
        store.link_name(ArtifactKind::Prompt, "system", second_hash).expect("move pointer");

        assert_eq!(store.resolve_name(ArtifactKind::Prompt, "system").expect("resolve second"), second_hash);
        assert_eq!(store.get(first_hash).expect("old artifact remains"), first);
        assert_eq!(store.get(second_hash).expect("new artifact remains"), second);
    }

    #[test]
    fn artifact_store_reports_missing_and_unsafe_name_without_jsonl_side_effects() {
        let tempdir = tempfile::tempdir().expect("temp artifact store");
        let store = ArtifactStore::new(tempdir.path());
        let missing = ArtifactHash::digest(b"missing");
        assert!(matches!(store.get(missing), Err(ArtifactStoreError::Missing { .. })));
        assert!(matches!(
            store.link_name(ArtifactKind::Prompt, "../session.jsonl", missing),
            Err(ArtifactStoreError::UnsafeName { .. })
        ));
        assert!(!tempdir.path().join("session.jsonl").exists());
    }

    #[test]
    fn receipt_references_cover_model_tool_session_and_review_roles() {
        let artifacts = [
            (ReceiptArtifactRole::ModelRequest, ArtifactKind::ModelRequest),
            (ReceiptArtifactRole::ToolDescriptor, ArtifactKind::ToolDescriptor),
            (ReceiptArtifactRole::SessionBlock, ArtifactKind::SessionBlock),
            (ReceiptArtifactRole::ReviewArtifact, ArtifactKind::Prompt),
        ]
        .into_iter()
        .map(|(role, kind)| {
            let (envelope, hash) = canonicalize_artifact(
                kind,
                RedactionClass::Public,
                json!({"kind": kind.as_str(), "role": format!("{role:?}")}),
                [],
            )
            .expect("canonicalize receipt artifact");
            ReceiptArtifactRef::from_envelope(role, hash, &envelope)
        })
        .collect::<Vec<_>>();

        let receipt = ReceiptArtifacts { artifacts }.sorted();
        let json = serde_json::to_value(&receipt).expect("receipt artifacts serialize");
        assert_eq!(json["artifacts"].as_array().expect("artifact refs").len(), 4);
        assert!(json.to_string().contains("model-request"));
        assert!(json.to_string().contains("review-artifact"));
    }

    #[test]
    fn inspect_summary_redacts_payload_unless_public() {
        let tempdir = tempfile::tempdir().expect("temp artifact store");
        let store = ArtifactStore::new(tempdir.path());
        let (public, public_hash) =
            canonicalize_artifact(ArtifactKind::Prompt, RedactionClass::Public, json!({"content":"safe"}), [])
                .expect("public artifact");
        let (redacted, redacted_hash) = canonicalize_artifact(
            ArtifactKind::ModelRequest,
            RedactionClass::RedactedPayload,
            json!({"body":"provider payload"}),
            [],
        )
        .expect("redacted artifact");
        store.put(&public).expect("store public");
        store.put(&redacted).expect("store redacted");

        let public_summary = store.inspect(public_hash).expect("inspect public");
        assert_eq!(public_summary.payload, Some(json!({"content":"safe"})));
        assert_eq!(public_summary.redacted_reason, None);

        let redacted_summary = store.inspect(redacted_hash).expect("inspect redacted");
        assert_eq!(redacted_summary.payload, None);
        assert!(redacted_summary.redacted_reason.expect("redacted reason").contains("RedactedPayload"));
    }

    #[test]
    fn pure_cache_key_ignores_declaration_ordering_and_changes_on_inputs() {
        let first = test_declaration([ArtifactHash::digest(b"b"), ArtifactHash::digest(b"a")], ["PATH", "HOME"]);
        let reordered = test_declaration([ArtifactHash::digest(b"a"), ArtifactHash::digest(b"b")], ["HOME", "PATH"]);
        let changed = test_declaration([ArtifactHash::digest(b"changed")], ["HOME", "PATH"]);

        let first_key = first.cache_key().expect("first key");
        assert_eq!(first_key, reordered.cache_key().expect("reordered key"));
        assert_ne!(first_key, changed.cache_key().expect("changed key"));
    }

    #[test]
    fn pure_result_cache_is_opt_in_and_records_hit_receipts() {
        let tempdir = tempfile::tempdir().expect("temp pure cache");
        let cache = PureResultCache::new(tempdir.path());
        let declaration = test_declaration([ArtifactHash::digest(b"input")], ["PATH"]);
        let eligibility = CacheEligibility::evaluate(declaration, []).expect("eligible cache");
        assert!(eligibility.is_allowed());
        let key = eligibility.key.expect("cache key");

        let (miss, miss_receipt) = cache.get(&eligibility).expect("cache miss");
        assert_eq!(miss, None);
        assert!(!miss_receipt.hit);
        assert_eq!(miss_receipt.key, Some(key));

        cache.put(key, &json!({"answer": 42})).expect("store pure result");
        let (hit, hit_receipt) = cache.get(&eligibility).expect("cache hit");
        assert_eq!(hit, Some(json!({"answer": 42})));
        assert!(hit_receipt.hit);
        assert_eq!(hit_receipt.denied_effects, Vec::new());
    }

    #[test]
    fn pure_result_cache_denies_hidden_effects_without_secret_values() {
        let declaration = test_declaration([ArtifactHash::digest(b"input")], ["PATH"]);
        let eligibility = CacheEligibility::evaluate(declaration, [
            EffectClass::Network,
            EffectClass::UndeclaredEnvironment,
            EffectClass::Network,
        ])
        .expect("denied cache");
        assert!(!eligibility.is_allowed());
        assert_eq!(eligibility.key, None);
        assert_eq!(eligibility.denied_effects, vec![EffectClass::UndeclaredEnvironment, EffectClass::Network]);

        let cache = PureResultCache::new(tempfile::tempdir().expect("temp pure cache").path());
        let (value, receipt) = cache.get(&eligibility).expect("denied lookup");
        assert_eq!(value, None);
        assert!(!receipt.hit);
        assert_eq!(receipt.denied_effects, eligibility.denied_effects);
    }

    #[test]
    fn pure_cache_rails_deny_shell_time_and_file_mutation_effects() {
        let declaration = test_declaration([ArtifactHash::digest(b"input")], ["PATH"]);
        let eligibility = CacheEligibility::evaluate(declaration, [
            EffectClass::Process,
            EffectClass::Time,
            EffectClass::FileMutation,
            EffectClass::Process,
        ])
        .expect("denied cache");

        assert_eq!(eligibility.key, None);
        assert_eq!(eligibility.denied_effects, vec![
            EffectClass::Time,
            EffectClass::Process,
            EffectClass::FileMutation
        ]);
    }

    #[test]
    fn pure_cache_rails_invalidate_on_file_input_changes() {
        let mut first = test_declaration([ArtifactHash::digest(b"artifact")], ["PATH"]);
        first.file_input_hashes = vec![ArtifactHash::digest(b"file-v1")];
        let mut changed = test_declaration([ArtifactHash::digest(b"artifact")], ["PATH"]);
        changed.file_input_hashes = vec![ArtifactHash::digest(b"file-v2")];

        assert_ne!(first.cache_key().expect("first key"), changed.cache_key().expect("changed key"));
    }

    fn test_declaration<const N: usize, const M: usize>(
        artifacts: [ArtifactHash; N],
        env: [&str; M],
    ) -> DeterministicInputDeclaration {
        DeterministicInputDeclaration {
            artifact_hashes: artifacts.into_iter().collect(),
            file_input_hashes: vec![ArtifactHash::digest(b"file")],
            env_allowlist: env.into_iter().map(str::to_owned).collect(),
            tool_version: "test-tool@1".to_owned(),
            effect_profile: EffectProfile::NoHiddenEffects,
        }
    }

    #[test]
    fn golden_hash_fixtures_cover_supported_artifact_kinds() {
        let fixtures = [
            (
                ArtifactKind::Prompt,
                RedactionClass::Public,
                json!({"messages":[{"content":"answer briefly","role":"system"}],"timestamp":"ignored"}),
                "b3:2e62319389a7864be995bc7b278f95fac28214bde8df79a8ffe2a3ea2a6203a1",
            ),
            (
                ArtifactKind::ToolDescriptor,
                RedactionClass::Public,
                json!({"name":"read_file","schema":{"properties":{"path":{"type":"string"}},"type":"object"}}),
                "b3:94e78f42d79ea47be7c1f9d153177ce251244d0daf445e673c61eb21616da791",
            ),
            (
                ArtifactKind::ModelRequest,
                RedactionClass::RedactedPayload,
                json!({"messages":[{"content":"hello","role":"user"}],"model":"gpt-5.5"}),
                "b3:9b65d4b7f600aa85ca8e1594200fde2760f0c4708d2a9d9fb10bb5723c83500f",
            ),
            (
                ArtifactKind::McpManifest,
                RedactionClass::Public,
                json!({"server":"ssg-onix","tools":["site_build","site_audit"]}),
                "b3:2d0a8e73d1c5837a14d3ebd6229878f6cf23041026d19562a9a71606bbbe3cd5",
            ),
            (
                ArtifactKind::PluginManifest,
                RedactionClass::MetadataOnly,
                json!({"kind":"extism","name":"review-gate","path":"/ignored/plugin.wasm"}),
                "b3:2a55953817f9892575b8449ff0eecd86f0a125a94cb173e39607e2b54aebc742",
            ),
            (
                ArtifactKind::SkillReference,
                RedactionClass::MetadataOnly,
                json!({"name":"cairn","version":"1","updated_at":"ignored"}),
                "b3:f3c5b29781629a865ed2bd444907badde994e57b1b20e50ab267d02a2fca929a",
            ),
            (
                ArtifactKind::SessionBlock,
                RedactionClass::Public,
                json!({"messages":[{"content":"hi","role":"user"}],"started_at":"ignored"}),
                "b3:63f105f560fbda3cc07233439cd22377cbdfe3cf58cdf0ecadf1d2b0ebfe1a80",
            ),
        ];

        for (kind, redaction, payload, expected_hash) in fixtures {
            let (_, hash) =
                canonicalize_artifact(kind, redaction, payload, []).unwrap_or_else(|_| panic!("{}", kind.as_str()));
            assert_eq!(hash.to_string(), expected_hash, "golden fixture for {}", kind.as_str());
        }
    }
}
