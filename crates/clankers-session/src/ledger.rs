//! Typed durable session ledger records.
//!
//! Ledger records are append-only structured facts written beside existing session transcripts.
//! They store safe identifiers and hashes, never raw provider payloads or secret-bearing tool
//! output.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use chrono::DateTime;
use chrono::Utc;
use clanker_message::transcript::MessageId;
use clankers_artifacts::ArtifactHash;
use clankers_artifacts::RedactionClass;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::error::Result;
use crate::error::session_err;

/// Current typed ledger schema version.
pub const LEDGER_SCHEMA_VERSION: u32 = 1;

/// One append-only typed session fact or opaque safe fallback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// allow justification: LedgerRecord preserves stable JSON shape; boxing would not reduce serialized payload size.
#[allow(
    clippy::large_enum_variant,
    reason = "LedgerRecord preserves stable JSON shape; boxing the typed variant would not reduce serialized payload size and risks API churn."
)]
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
            observed_at: crate::session_clock_now(),
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
            observed_at: crate::session_clock_now(),
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
    #[serde(default = "BTreeMap::new", skip_serializing_if = "BTreeMap::is_empty")]
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
    /// Cairn change/task/requirement fact.
    Cairn(CairnLedgerFact),
    /// Error fact with safe class/message.
    Error(ErrorLedgerFact),
    /// Artifact hash reference fact.
    ArtifactReference(ArtifactReferenceLedgerFact),
    /// Pending non-destructive change/refactor work fact.
    PendingChange(PendingChangeLedgerFact),
    /// Todo item lifecycle fact for structured work sessions.
    Todo(TodoLedgerFact),
    /// UCAN/runtime authorization decision fact.
    Authorization(AuthorizationLedgerFact),
}

impl LedgerPayload {
    fn sanitized(self) -> Self {
        match self {
            Self::Model(fact) => Self::Model(fact.sanitized()),
            Self::Tool(fact) => Self::Tool(fact.sanitized()),
            Self::Block(fact) => Self::Block(fact.sanitized()),
            Self::Review(fact) => Self::Review(fact.sanitized()),
            Self::Cairn(fact) => Self::Cairn(fact.sanitized()),
            Self::Error(fact) => Self::Error(fact.sanitized()),
            Self::ArtifactReference(fact) => Self::ArtifactReference(fact.sanitized()),
            Self::PendingChange(fact) => Self::PendingChange(fact.sanitized()),
            Self::Todo(fact) => Self::Todo(fact.sanitized()),
            Self::Authorization(fact) => Self::Authorization(fact.sanitized()),
        }
    }

    /// Borrow shared query fields for indexing without exposing payload content.
    #[must_use]
    pub fn query_fields(&self) -> &LedgerQueryFields {
        match self {
            Self::Model(fact) => &fact.query,
            Self::Tool(fact) => &fact.query,
            Self::Block(fact) => &fact.query,
            Self::Review(fact) => &fact.query,
            Self::Cairn(fact) => &fact.query,
            Self::Error(fact) => &fact.query,
            Self::ArtifactReference(fact) => &fact.query,
            Self::PendingChange(fact) => &fact.query,
            Self::Todo(fact) => &fact.query,
            Self::Authorization(fact) => &fact.query,
        }
    }
}

/// Shared query fields carried by typed facts.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LedgerQueryFields {
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub artifact_hashes: Vec<ArtifactHash>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crate_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_shape: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect_ability: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect_resource: Option<String>,
}

/// Query over safe ledger metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LedgerQuery {
    pub artifact_hash: Option<ArtifactHash>,
    pub tool_kind: Option<String>,
    pub error_class: Option<String>,
    pub crate_path: Option<String>,
    pub requirement_id: Option<String>,
    pub request_shape: Option<String>,
    pub authorization_status: Option<String>,
    pub effect_ability: Option<String>,
    pub effect_resource: Option<String>,
}

/// In-memory local index for typed session ledger records.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LedgerIndex {
    records: Vec<LedgerRecord>,
}

impl LedgerIndex {
    #[must_use]
    pub fn from_records(records: Vec<LedgerRecord>) -> Self {
        Self { records }
    }

    #[must_use]
    pub fn records(&self) -> &[LedgerRecord] {
        &self.records
    }

    #[must_use]
    pub fn query(&self, query: &LedgerQuery) -> Vec<&LedgerRecord> {
        self.records.iter().filter(|record| record_matches_query(record, query)).collect()
    }
}

fn record_matches_query(record: &LedgerRecord, query: &LedgerQuery) -> bool {
    let LedgerRecord::Typed(record) = record else {
        return false;
    };
    query_fields_match(record.payload.query_fields(), query)
}

fn query_fields_match(fields: &LedgerQueryFields, query: &LedgerQuery) -> bool {
    query.artifact_hash.is_none_or(|hash| fields.artifact_hashes.contains(&hash))
        && optional_query_text_matches(&query.tool_kind, &fields.tool_kind)
        && optional_query_text_matches(&query.error_class, &fields.error_class)
        && optional_query_text_matches(&query.crate_path, &fields.crate_path)
        && optional_query_text_matches(&query.requirement_id, &fields.requirement_id)
        && optional_query_text_matches(&query.request_shape, &fields.request_shape)
        && optional_query_text_matches(&query.authorization_status, &fields.authorization_status)
        && optional_query_text_matches(&query.effect_ability, &fields.effect_ability)
        && optional_query_text_matches(&query.effect_resource, &fields.effect_resource)
}

fn optional_query_text_matches(expected: &Option<String>, actual: &Option<String>) -> bool {
    expected.as_ref().is_none_or(|expected| actual.as_ref() == Some(expected))
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
        self.authorization_status = self.authorization_status.map(sanitize_ledger_text);
        self.effect_ability = self.effect_ability.map(sanitize_ledger_text);
        self.effect_resource = self.effect_resource.map(sanitize_ledger_text);
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
pub struct CairnLedgerFact {
    pub change: String,
    pub task: String,
    pub status: String,
    pub query: LedgerQueryFields,
}

impl CairnLedgerFact {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingChangeLedgerFact {
    pub change_id: String,
    pub scope: String,
    pub status: String,
    pub query: LedgerQueryFields,
}

impl PendingChangeLedgerFact {
    fn sanitized(self) -> Self {
        Self {
            change_id: sanitize_ledger_text(self.change_id),
            scope: sanitize_ledger_text(self.scope),
            status: sanitize_ledger_text(self.status),
            query: self.query.sanitized(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoLedgerFact {
    pub todo_id: String,
    pub content: String,
    pub status: String,
    pub query: LedgerQueryFields,
}

impl TodoLedgerFact {
    fn sanitized(self) -> Self {
        Self {
            todo_id: sanitize_ledger_text(self.todo_id),
            content: sanitize_ledger_text(self.content),
            status: sanitize_ledger_text(self.status),
            query: self.query.sanitized(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationLedgerFact {
    pub decision_id: String,
    pub status: String,
    pub effect_ability: String,
    pub effect_resource: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer_did: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audience_did: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proof_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub denial_class: Option<String>,
    pub query: LedgerQueryFields,
}

impl AuthorizationLedgerFact {
    fn sanitized(mut self) -> Self {
        self.decision_id = sanitize_ledger_text(self.decision_id);
        self.status = sanitize_ledger_text(self.status);
        self.effect_ability = sanitize_ledger_text(self.effect_ability);
        self.effect_resource = sanitize_ledger_text(self.effect_resource);
        self.issuer_did = self.issuer_did.map(sanitize_ledger_text);
        self.audience_did = self.audience_did.map(sanitize_ledger_text);
        self.proof_reference = self.proof_reference.map(sanitize_ledger_text);
        self.denial_class = self.denial_class.map(sanitize_ledger_text);
        self.query = self.query.sanitized();
        if self.query.authorization_status.is_none() {
            self.query.authorization_status = Some(self.status.clone());
        }
        if self.query.effect_ability.is_none() {
            self.query.effect_ability = Some(self.effect_ability.clone());
        }
        if self.query.effect_resource.is_none() {
            self.query.effect_resource = Some(self.effect_resource.clone());
        }
        self
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
        .unwrap_or_else(BTreeMap::new);
    LedgerRecord::opaque(id, original_kind, schema_version, safe_metadata)
}

/// Append one typed ledger record to a JSONL ledger file.
pub fn append_ledger_record(path: &Path, record: &LedgerRecord) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(session_err)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path).map_err(session_err)?;
    let line = serde_json::to_string(record).map_err(session_err)?;
    writeln!(file, "{line}").map_err(session_err)?;
    Ok(())
}

/// Read all typed ledger records from a JSONL ledger file.
pub fn read_ledger_records(path: &Path) -> Result<Vec<LedgerRecord>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read_to_string(path).map_err(session_err)?;
    let mut records = Vec::with_capacity(contents.lines().count());
    for line in contents.lines() {
        if line.trim().is_empty() {
            continue;
        }
        records.push(parse_ledger_record_compat(&line)?);
    }
    Ok(records)
}

fn parse_ledger_record_compat(line: &str) -> Result<LedgerRecord> {
    let value: Value = serde_json::from_str(line).map_err(session_err)?;
    let parsed = serde_json::from_value::<LedgerRecord>(value.clone());
    match parsed {
        Ok(LedgerRecord::Typed(record)) if record.schema_version == LEDGER_SCHEMA_VERSION => {
            Ok(LedgerRecord::Typed(record))
        }
        Ok(LedgerRecord::Opaque(record)) => Ok(LedgerRecord::Opaque(record)),
        _ => {
            let id = value.get("id").and_then(Value::as_str).unwrap_or("opaque-ledger-record");
            Ok(opaque_from_unknown_json(id, &value))
        }
    }
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
    fn append_read_round_trips_all_typed_ledger_record_kinds() {
        let path = std::env::temp_dir().join(format!("clankers-ledger-round-trip-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let artifact = ArtifactHash::digest(b"artifact-reference");
        let records = vec![
            LedgerRecord::typed(
                "model",
                LedgerPayload::Model(ModelLedgerFact {
                    message_id: Some(MessageId::new("model-message")),
                    model: "gpt-test".to_owned(),
                    provider: "openai".to_owned(),
                    query: LedgerQueryFields::default(),
                    redaction_class: RedactionClass::MetadataOnly,
                }),
            ),
            LedgerRecord::typed(
                "tool",
                LedgerPayload::Tool(ToolLedgerFact {
                    call_id: "call-1".to_owned(),
                    tool_name: "terminal".to_owned(),
                    status: "ok".to_owned(),
                    query: LedgerQueryFields {
                        artifact_hashes: vec![artifact],
                        ..LedgerQueryFields::default()
                    },
                    redaction_class: RedactionClass::MetadataOnly,
                }),
            ),
            LedgerRecord::typed(
                "block",
                LedgerPayload::Block(BlockLedgerFact {
                    block_id: "block-1".to_owned(),
                    block_kind: "assistant".to_owned(),
                    finalized_hash: Some(artifact),
                    query: LedgerQueryFields::default(),
                }),
            ),
            LedgerRecord::typed(
                "review",
                LedgerPayload::Review(ReviewLedgerFact {
                    review_id: "review-1".to_owned(),
                    verdict: "pass".to_owned(),
                    finding_count: 0,
                    query: LedgerQueryFields {
                        artifact_hashes: vec![artifact],
                        ..LedgerQueryFields::default()
                    },
                }),
            ),
            LedgerRecord::typed(
                "cairn",
                LedgerPayload::Cairn(CairnLedgerFact {
                    change: "add-typed-durable-session-ledger".to_owned(),
                    task: "records".to_owned(),
                    status: "done".to_owned(),
                    query: LedgerQueryFields {
                        artifact_hashes: vec![artifact],
                        ..LedgerQueryFields::default()
                    },
                }),
            ),
            LedgerRecord::typed(
                "error",
                LedgerPayload::Error(ErrorLedgerFact {
                    class: "E_TEST".to_owned(),
                    safe_message: "safe error".to_owned(),
                    query: LedgerQueryFields::default(),
                }),
            ),
            LedgerRecord::typed(
                "artifact",
                LedgerPayload::ArtifactReference(ArtifactReferenceLedgerFact {
                    artifact_hash: artifact,
                    artifact_kind: "result".to_owned(),
                    query: LedgerQueryFields::default(),
                }),
            ),
            LedgerRecord::typed(
                "authorization",
                LedgerPayload::Authorization(AuthorizationLedgerFact {
                    decision_id: "decision-1".to_owned(),
                    status: "allowed".to_owned(),
                    effect_ability: "file.read".to_owned(),
                    effect_resource: "file/src/lib.rs".to_owned(),
                    issuer_did: Some("did:key:issuer".to_owned()),
                    audience_did: Some("did:key:audience".to_owned()),
                    proof_reference: Some("proof-ref".to_owned()),
                    denial_class: None,
                    query: LedgerQueryFields::default(),
                }),
            ),
        ];

        for record in &records {
            append_ledger_record(&path, record).expect("append record");
        }
        let restored = read_ledger_records(&path).expect("read ledger records");
        let _ = std::fs::remove_file(&path);

        assert_eq!(restored, records);
    }

    #[test]
    fn local_ledger_index_queries_safe_metadata_fields() {
        let artifact = ArtifactHash::digest(b"query-artifact");
        let matching = LedgerRecord::typed(
            "tool-query",
            LedgerPayload::Tool(ToolLedgerFact {
                call_id: "call-1".to_owned(),
                tool_name: "terminal".to_owned(),
                status: "ok".to_owned(),
                query: LedgerQueryFields {
                    artifact_hashes: vec![artifact],
                    tool_kind: Some("shell".to_owned()),
                    crate_path: Some("crates/clankers-session".to_owned()),
                    requirement_id: Some("typed-durable-session-ledger.query".to_owned()),
                    request_shape: Some("tool-call".to_owned()),
                    ..LedgerQueryFields::default()
                },
                redaction_class: RedactionClass::MetadataOnly,
            }),
        );
        let error = LedgerRecord::typed(
            "error-query",
            LedgerPayload::Error(ErrorLedgerFact {
                class: "io".to_owned(),
                safe_message: "safe".to_owned(),
                query: LedgerQueryFields {
                    error_class: Some("io".to_owned()),
                    ..LedgerQueryFields::default()
                },
            }),
        );
        let index = LedgerIndex::from_records(vec![matching, error]);

        assert_eq!(
            index
                .query(&LedgerQuery {
                    artifact_hash: Some(artifact),
                    tool_kind: Some("shell".to_owned()),
                    requirement_id: Some("typed-durable-session-ledger.query".to_owned()),
                    ..LedgerQuery::default()
                })
                .len(),
            1
        );
        assert_eq!(
            index
                .query(&LedgerQuery {
                    error_class: Some("io".to_owned()),
                    ..LedgerQuery::default()
                })
                .len(),
            1
        );
        assert!(
            index
                .query(&LedgerQuery {
                    request_shape: Some("missing".to_owned()),
                    ..LedgerQuery::default()
                })
                .is_empty()
        );
    }

    #[test]
    fn authorization_facts_are_queryable_and_redact_secret_markers() {
        let allowed = LedgerRecord::typed(
            "auth-allowed",
            LedgerPayload::Authorization(AuthorizationLedgerFact {
                decision_id: "decision-allowed".to_owned(),
                status: "allowed".to_owned(),
                effect_ability: "file.read".to_owned(),
                effect_resource: "file/src/lib.rs".to_owned(),
                issuer_did: Some("did:key:issuer".to_owned()),
                audience_did: Some("did:key:audience".to_owned()),
                proof_reference: Some("proof-ref".to_owned()),
                denial_class: None,
                query: LedgerQueryFields::default(),
            }),
        );
        let denied = LedgerRecord::typed(
            "auth-denied",
            LedgerPayload::Authorization(AuthorizationLedgerFact {
                decision_id: "decision-denied".to_owned(),
                status: "denied".to_owned(),
                effect_ability: "tool.execute".to_owned(),
                effect_resource: "token-bearing-secret-command".to_owned(),
                issuer_did: None,
                audience_did: None,
                proof_reference: None,
                denial_class: Some("missing_authority".to_owned()),
                query: LedgerQueryFields::default(),
            }),
        );
        let index = LedgerIndex::from_records(vec![allowed, denied]);

        let denials = index.query(&LedgerQuery {
            authorization_status: Some("denied".to_owned()),
            effect_ability: Some("tool.execute".to_owned()),
            ..LedgerQuery::default()
        });
        assert_eq!(denials.len(), 1);
        let LedgerRecord::Typed(record) = denials[0] else {
            panic!("expected typed record");
        };
        let LedgerPayload::Authorization(fact) = &record.payload else {
            panic!("expected authorization fact");
        };
        assert_eq!(fact.effect_resource, "[redacted-secret-marker]");
        assert_eq!(fact.query.authorization_status.as_deref(), Some("denied"));
        assert_eq!(fact.query.effect_ability.as_deref(), Some("tool.execute"));
        assert_eq!(fact.denial_class.as_deref(), Some("missing_authority"));
    }

    #[test]
    fn structured_work_facts_capture_pending_changes_and_todos() {
        let req = "typed-durable-session-ledger.structured-work";
        let records = vec![
            LedgerRecord::typed(
                "pending-change",
                LedgerPayload::PendingChange(PendingChangeLedgerFact {
                    change_id: "add-typed-durable-session-ledger".to_owned(),
                    scope: "session/refactor".to_owned(),
                    status: "pending".to_owned(),
                    query: LedgerQueryFields {
                        requirement_id: Some(req.to_owned()),
                        ..LedgerQueryFields::default()
                    },
                }),
            ),
            LedgerRecord::typed(
                "todo",
                LedgerPayload::Todo(TodoLedgerFact {
                    todo_id: "todo-1".to_owned(),
                    content: "record structured todo".to_owned(),
                    status: "in-progress".to_owned(),
                    query: LedgerQueryFields {
                        requirement_id: Some(req.to_owned()),
                        ..LedgerQueryFields::default()
                    },
                }),
            ),
        ];

        for record in records {
            let LedgerRecord::Typed(typed) = record else {
                panic!("expected typed work fact");
            };
            assert_eq!(typed.schema_version, LEDGER_SCHEMA_VERSION);
            assert_eq!(typed.payload.query_fields().requirement_id.as_deref(), Some(req));
        }
    }

    #[test]
    fn read_ledger_records_migrates_future_or_unknown_records_to_opaque() {
        let path = std::env::temp_dir().join(format!("clankers-ledger-migration-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        std::fs::write(
            &path,
            r#"{"record_kind":"future_kind","id":"future-1","schema_version":7,"safe_metadata":{"kind":"future","authorization":"Bearer nope"},"raw_payload":"drop-me"}
{"record_kind":"typed","id":"typed-future","schema_version":99,"created_at":"2026-05-17T00:00:00Z","safe_metadata":{"note":"future typed"},"payload":{"payload_kind":"error","class":"future","safe_message":"future","query":{"artifact_hashes":[]}}}
"#,
        )
        .expect("write fixture");

        let records = read_ledger_records(&path).expect("read compat records");
        let _ = std::fs::remove_file(&path);

        assert_eq!(records.len(), 2);
        for record in records {
            let LedgerRecord::Opaque(opaque) = record else {
                panic!("future record should become opaque");
            };
            let serialized = serde_json::to_string(&opaque).expect("opaque json");
            assert!(!serialized.contains("Bearer"));
            assert!(!serialized.contains("drop-me"));
        }
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
