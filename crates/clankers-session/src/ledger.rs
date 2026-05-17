//! Typed durable session ledger records.
//!
//! Ledger records are append-only structured facts written beside existing session transcripts.
//! They store safe identifiers and hashes, never raw provider payloads or secret-bearing tool
//! output.

use std::collections::BTreeMap;

use chrono::DateTime;
use chrono::Utc;
use clanker_message::MessageId;
use clankers_artifacts::ArtifactHash;
use clankers_artifacts::RedactionClass;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

/// Current typed ledger schema version.
pub const LEDGER_SCHEMA_VERSION: u32 = 1;

/// One append-only typed session fact or opaque safe fallback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "record_kind", rename_all = "snake_case")]
pub enum LedgerRecord {
    /// Current schema record with typed payload.
    Typed(TypedLedgerRecord),
    /// Unknown/future record preserved as safe metadata without payload interpretation.
    Opaque(OpaqueLedgerRecord),
}

impl LedgerRecord {
    /// Construct a current-schema typed record.
    #[must_use]
    pub fn typed(id: impl Into<String>, payload: LedgerPayload) -> Self {
        Self::Typed(TypedLedgerRecord::new(id, payload))
    }

    /// Construct a safe opaque fallback for unknown record kinds or future versions.
    #[must_use]
    pub fn opaque(
        id: impl Into<String>,
        original_kind: impl Into<String>,
        schema_version: u32,
        safe_metadata: BTreeMap<String, String>,
    ) -> Self {
        Self::Opaque(OpaqueLedgerRecord {
            id: sanitize_ledger_text(id.into()),
            original_kind: sanitize_ledger_text(original_kind.into()),
            schema_version,
            safe_metadata: sanitize_metadata_map(safe_metadata),
            observed_at: Utc::now(),
        })
    }

    /// Borrow the stable record ID.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Typed(record) => &record.id,
            Self::Opaque(record) => &record.id,
        }
    }

    /// Return true when the record can be interpreted by this reader.
    #[must_use]
    pub fn is_typed(&self) -> bool {
        matches!(self, Self::Typed(_))
    }
}

/// Current typed ledger record envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedLedgerRecord {
    /// Stable record ID.
    pub id: String,
    /// Current schema version for this record envelope.
    pub schema_version: u32,
    /// Time the fact was observed/written.
    pub observed_at: DateTime<Utc>,
    /// Typed record payload.
    pub payload: LedgerPayload,
}

impl TypedLedgerRecord {
    /// Create a current-schema record and sanitize payload metadata.
    #[must_use]
    pub fn new(id: impl Into<String>, payload: LedgerPayload) -> Self {
        Self {
            id: sanitize_ledger_text(id.into()),
            schema_version: LEDGER_SCHEMA_VERSION,
            observed_at: Utc::now(),
            payload: payload.sanitized(),
        }
    }
}

/// Unknown or future ledger record fallback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpaqueLedgerRecord {
    /// Stable safe record ID.
    pub id: String,
    /// Original kind label, sanitized.
    pub original_kind: String,
    /// Original schema version.
    pub schema_version: u32,
    /// Safe queryable metadata only; raw payload is intentionally not retained.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub safe_metadata: BTreeMap<String, String>,
    /// Time the opaque record was observed.
    pub observed_at: DateTime<Utc>,
}

/// Typed ledger fact payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LedgerPayload {
    /// Model request/response shape without raw provider body.
    Model(ModelLedgerFact),
    /// Tool call/result shape without raw tool payload.
    Tool(ToolLedgerFact),
    /// TUI/conversation block lifecycle fact.
    Block(BlockLedgerFact),
    /// Review finding/receipt fact.
    Review(ReviewLedgerFact),
    /// OpenSpec change/task/requirement fact.
    OpenSpec(OpenSpecLedgerFact),
    /// Error fact with safe class/message.
    Error(ErrorLedgerFact),
    /// Artifact hash reference fact.
    ArtifactReference(ArtifactReferenceLedgerFact),
}

impl LedgerPayload {
    fn sanitized(self) -> Self {
        match self {
            Self::Model(fact) => Self::Model(fact.sanitized()),
            Self::Tool(fact) => Self::Tool(fact.sanitized()),
            Self::Block(fact) => Self::Block(fact.sanitized()),
            Self::Review(fact) => Self::Review(fact.sanitized()),
            Self::OpenSpec(fact) => Self::OpenSpec(fact.sanitized()),
            Self::Error(fact) => Self::Error(fact.sanitized()),
            Self::ArtifactReference(fact) => Self::ArtifactReference(fact.sanitized()),
        }
    }
}

/// Shared query fields carried by typed facts.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LedgerQueryFields {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_hashes: Vec<ArtifactHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crate_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_shape: Option<String>,
}

impl LedgerQueryFields {
    fn sanitized(mut self) -> Self {
        self.artifact_hashes.sort_by_key(|hash| hash.hex());
        self.artifact_hashes.dedup();
        self.tool_kind = self.tool_kind.map(sanitize_ledger_text);
        self.error_class = self.error_class.map(sanitize_ledger_text);
        self.crate_path = self.crate_path.map(sanitize_ledger_text);
        self.requirement_id = self.requirement_id.map(sanitize_ledger_text);
        self.request_shape = self.request_shape.map(sanitize_ledger_text);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelLedgerFact {
    pub message_id: Option<MessageId>,
    pub model: String,
    pub provider: String,
    pub query: LedgerQueryFields,
    pub redaction_class: RedactionClass,
}

impl ModelLedgerFact {
    fn sanitized(self) -> Self {
        Self {
            model: sanitize_ledger_text(self.model),
            provider: sanitize_ledger_text(self.provider),
            query: self.query.sanitized(),
            ..self
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolLedgerFact {
    pub call_id: String,
    pub tool_name: String,
    pub status: String,
    pub query: LedgerQueryFields,
    pub redaction_class: RedactionClass,
}

impl ToolLedgerFact {
    fn sanitized(self) -> Self {
        Self {
            call_id: sanitize_ledger_text(self.call_id),
            tool_name: sanitize_ledger_text(self.tool_name),
            status: sanitize_ledger_text(self.status),
            query: self.query.sanitized(),
            redaction_class: self.redaction_class,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockLedgerFact {
    pub block_id: String,
    pub block_kind: String,
    pub finalized_hash: Option<ArtifactHash>,
    pub query: LedgerQueryFields,
}

impl BlockLedgerFact {
    fn sanitized(self) -> Self {
        Self {
            block_id: sanitize_ledger_text(self.block_id),
            block_kind: sanitize_ledger_text(self.block_kind),
            finalized_hash: self.finalized_hash,
            query: self.query.sanitized(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewLedgerFact {
    pub review_id: String,
    pub verdict: String,
    pub finding_count: u32,
    pub query: LedgerQueryFields,
}

impl ReviewLedgerFact {
    fn sanitized(self) -> Self {
        Self {
            review_id: sanitize_ledger_text(self.review_id),
            verdict: sanitize_ledger_text(self.verdict),
            finding_count: self.finding_count,
            query: self.query.sanitized(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenSpecLedgerFact {
    pub change: String,
    pub task: String,
    pub status: String,
    pub query: LedgerQueryFields,
}

impl OpenSpecLedgerFact {
    fn sanitized(self) -> Self {
        Self {
            change: sanitize_ledger_text(self.change),
            task: sanitize_ledger_text(self.task),
            status: sanitize_ledger_text(self.status),
            query: self.query.sanitized(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorLedgerFact {
    pub class: String,
    pub safe_message: String,
    pub query: LedgerQueryFields,
}

impl ErrorLedgerFact {
    fn sanitized(self) -> Self {
        Self {
            class: sanitize_ledger_text(self.class),
            safe_message: sanitize_ledger_text(self.safe_message),
            query: self.query.sanitized(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactReferenceLedgerFact {
    pub artifact_hash: ArtifactHash,
    pub artifact_kind: String,
    pub query: LedgerQueryFields,
}

impl ArtifactReferenceLedgerFact {
    fn sanitized(self) -> Self {
        Self {
            artifact_hash: self.artifact_hash,
            artifact_kind: sanitize_ledger_text(self.artifact_kind),
            query: self.query.sanitized(),
        }
    }
}

fn sanitize_metadata_map(metadata: BTreeMap<String, String>) -> BTreeMap<String, String> {
    metadata
        .into_iter()
        .map(|(key, value)| (sanitize_ledger_text(key), sanitize_ledger_text(value)))
        .collect()
}

fn sanitize_ledger_text(value: String) -> String {
    let lowered = value.to_ascii_lowercase();
    if ["authorization", "bearer", "password", "secret", "token", "api_key"]
        .iter()
        .any(|marker| lowered.contains(marker))
    {
        "[redacted-secret-marker]".to_owned()
    } else {
        value.chars().filter(|character| !character.is_control()).collect()
    }
}

/// Convert unknown raw JSON into an opaque safe fallback record.
#[must_use]
pub fn opaque_from_unknown_json(id: impl Into<String>, value: &Value) -> LedgerRecord {
    let original_kind = value.get("record_kind").and_then(Value::as_str).unwrap_or("unknown").to_owned();
    let schema_version = value
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
        .unwrap_or(0);
    let safe_metadata = value
        .get("safe_metadata")
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .filter_map(|(key, value)| value.as_str().map(|text| (key.clone(), text.to_owned())))
                .collect()
        })
        .unwrap_or_default();
    LedgerRecord::opaque(id, original_kind, schema_version, safe_metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_ledger_record_carries_schema_kind_and_redacted_query_fields() {
        let hash = ArtifactHash::digest(b"model request shape");
        let record = LedgerRecord::typed(
            "model-1",
            LedgerPayload::Model(ModelLedgerFact {
                message_id: Some(MessageId::new("msg-1")),
                model: "gpt-test".to_owned(),
                provider: "provider-token-value".to_owned(),
                query: LedgerQueryFields {
                    artifact_hashes: vec![hash, hash],
                    request_shape: Some("chat-completions".to_owned()),
                    ..LedgerQueryFields::default()
                },
                redaction_class: RedactionClass::MetadataOnly,
            }),
        );

        let LedgerRecord::Typed(typed) = record else {
            panic!("expected typed record");
        };
        assert_eq!(typed.schema_version, LEDGER_SCHEMA_VERSION);
        let LedgerPayload::Model(model) = typed.payload else {
            panic!("expected model fact");
        };
        assert_eq!(model.provider, "[redacted-secret-marker]");
        assert_eq!(model.query.artifact_hashes, vec![hash]);
    }

    #[test]
    fn opaque_unknown_fallback_preserves_safe_metadata_without_raw_payload() {
        let unknown = serde_json::json!({
            "record_kind": "future_tool",
            "schema_version": 99,
            "safe_metadata": {
                "tool": "search",
                "authorization": "Bearer should-not-survive"
            },
            "raw_payload": "not copied"
        });
        let opaque = opaque_from_unknown_json("opaque-1", &unknown);

        let LedgerRecord::Opaque(record) = opaque else {
            panic!("expected opaque record");
        };
        assert_eq!(record.original_kind, "future_tool");
        assert_eq!(record.schema_version, 99);
        assert_eq!(record.safe_metadata.get("tool"), Some(&"search".to_owned()));
        let serialized = serde_json::to_string(&record).expect("opaque json");
        assert!(!serialized.contains("Bearer"));
        assert!(!serialized.contains("should-not-survive"));
        assert!(!serialized.contains("raw_payload"));
    }
}
