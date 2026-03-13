//! Automerge document storage for sessions.
//!
//! Replaces JSONL append-only files with a single Automerge document per session.
//! Messages are stored in a map keyed by MessageId, annotations in an ordered list.
//! The document schema is:
//!
//! ```text
//! {
//!   "header": { session_id, created_at, cwd, model, version, ... },
//!   "messages": { "<id>": { parent_id, message_json, timestamp } },
//!   "annotations": [ { kind, ... }, ... ]
//! }
//! ```

use std::path::Path;

use automerge::transaction::Transactable;
use automerge::AutoCommit;
use automerge::ObjType;
use automerge::ReadDoc;
use automerge::Value;
use clankers_message::MessageId;

use crate::entry::BranchEntry;
use crate::entry::CompactionEntry;
use crate::entry::CustomEntry;
use crate::entry::HeaderEntry;
use crate::entry::LabelEntry;
use crate::entry::MessageEntry;
use crate::entry::ModelChangeEntry;
use crate::entry::ResumeEntry;
use crate::entry::SessionEntry;
use crate::error::Result;
use crate::error::session_err;

// --- Document key constants ---

const KEY_HEADER: &str = "header";
const KEY_MESSAGES: &str = "messages";
const KEY_ANNOTATIONS: &str = "annotations";

// Header fields
const H_SESSION_ID: &str = "session_id";
const H_CREATED_AT: &str = "created_at";
const H_CWD: &str = "cwd";
const H_MODEL: &str = "model";
const H_VERSION: &str = "version";
const H_AGENT: &str = "agent";
const H_PARENT_SESSION_ID: &str = "parent_session_id";
const H_WORKTREE_PATH: &str = "worktree_path";
const H_WORKTREE_BRANCH: &str = "worktree_branch";

// Message fields
const M_PARENT_ID: &str = "parent_id";
const M_MESSAGE_JSON: &str = "message_json";
const M_TIMESTAMP: &str = "timestamp";

// Annotation fields
const A_KIND: &str = "kind";

/// Create a new Automerge document initialized with the session header.
pub fn create_document(header: &HeaderEntry) -> Result<AutoCommit> {
    let mut doc = AutoCommit::new();

    // Root-level maps
    let header_obj = doc.put_object(automerge::ROOT, KEY_HEADER, ObjType::Map).map_err(session_err)?;
    doc.put_object(automerge::ROOT, KEY_MESSAGES, ObjType::Map)
        .map_err(session_err)?;
    doc.put_object(automerge::ROOT, KEY_ANNOTATIONS, ObjType::List)
        .map_err(session_err)?;

    // Populate header
    doc.put(&header_obj, H_SESSION_ID, header.session_id.as_str())
        .map_err(session_err)?;
    doc.put(&header_obj, H_CREATED_AT, header.created_at.to_rfc3339().as_str())
        .map_err(session_err)?;
    doc.put(&header_obj, H_CWD, header.cwd.as_str())
        .map_err(session_err)?;
    doc.put(&header_obj, H_MODEL, header.model.as_str())
        .map_err(session_err)?;
    doc.put(&header_obj, H_VERSION, header.version.as_str())
        .map_err(session_err)?;

    if let Some(agent) = &header.agent {
        doc.put(&header_obj, H_AGENT, agent.as_str())
            .map_err(session_err)?;
    }
    if let Some(parent) = &header.parent_session_id {
        doc.put(&header_obj, H_PARENT_SESSION_ID, parent.as_str())
            .map_err(session_err)?;
    }
    if let Some(wt_path) = &header.worktree_path {
        doc.put(&header_obj, H_WORKTREE_PATH, wt_path.as_str())
            .map_err(session_err)?;
    }
    if let Some(wt_branch) = &header.worktree_branch {
        doc.put(&header_obj, H_WORKTREE_BRANCH, wt_branch.as_str())
            .map_err(session_err)?;
    }

    Ok(doc)
}

/// Insert a message into the Automerge document's messages map.
pub fn put_message(doc: &mut AutoCommit, entry: &MessageEntry) -> Result<()> {
    let messages_obj = doc
        .get(automerge::ROOT, KEY_MESSAGES)
        .map_err(session_err)?
        .and_then(|(val, id)| if matches!(val, Value::Object(ObjType::Map)) { Some(id) } else { None })
        .ok_or_else(|| session_err("messages map not found in document"))?;

    let msg_obj = doc
        .put_object(&messages_obj, entry.id.0.as_str(), ObjType::Map)
        .map_err(session_err)?;

    if let Some(parent_id) = &entry.parent_id {
        doc.put(&msg_obj, M_PARENT_ID, parent_id.0.as_str())
            .map_err(session_err)?;
    }

    let message_json = serde_json::to_string(&entry.message).map_err(session_err)?;
    doc.put(&msg_obj, M_MESSAGE_JSON, message_json.as_str())
        .map_err(session_err)?;

    doc.put(&msg_obj, M_TIMESTAMP, entry.timestamp.to_rfc3339().as_str())
        .map_err(session_err)?;

    Ok(())
}

/// Append an annotation to the Automerge document's annotations list.
pub fn put_annotation(doc: &mut AutoCommit, annotation: &AnnotationEntry) -> Result<()> {
    let annotations_obj = doc
        .get(automerge::ROOT, KEY_ANNOTATIONS)
        .map_err(session_err)?
        .and_then(|(val, id)| if matches!(val, Value::Object(ObjType::List)) { Some(id) } else { None })
        .ok_or_else(|| session_err("annotations list not found in document"))?;

    let len = doc.length(&annotations_obj);
    let ann_obj = doc
        .insert_object(&annotations_obj, len, ObjType::Map)
        .map_err(session_err)?;

    // Serialize the annotation as JSON and store it — same rationale as message_json:
    // annotations are write-once, never partially merged.
    let json = serde_json::to_string(annotation).map_err(session_err)?;
    doc.put(&ann_obj, A_KIND, annotation.kind_str())
        .map_err(session_err)?;
    doc.put(&ann_obj, "data", json.as_str()).map_err(session_err)?;

    Ok(())
}

/// Read the session header from an Automerge document.
pub fn read_header(doc: &AutoCommit) -> Result<HeaderEntry> {
    let header_obj = doc
        .get(automerge::ROOT, KEY_HEADER)
        .map_err(session_err)?
        .and_then(|(val, id)| if matches!(val, Value::Object(ObjType::Map)) { Some(id) } else { None })
        .ok_or_else(|| session_err("header not found in document"))?;

    let get_str = |key: &str| -> Result<String> {
        doc.get(&header_obj, key)
            .map_err(session_err)?
            .and_then(|(val, _)| if let Value::Scalar(s) = val { s.to_str().map(String::from) } else { None })
            .ok_or_else(|| session_err(format!("header field '{}' not found", key)))
    };

    let get_opt_str = |key: &str| -> Result<Option<String>> {
        match doc.get(&header_obj, key).map_err(session_err)? {
            Some((Value::Scalar(s), _)) => Ok(s.to_str().map(String::from)),
            _ => Ok(None),
        }
    };

    let created_at_str = get_str(H_CREATED_AT)?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(session_err)?
        .with_timezone(&chrono::Utc);

    Ok(HeaderEntry {
        session_id: get_str(H_SESSION_ID)?,
        created_at,
        cwd: get_str(H_CWD)?,
        model: get_str(H_MODEL)?,
        version: get_str(H_VERSION)?,
        agent: get_opt_str(H_AGENT)?,
        parent_session_id: get_opt_str(H_PARENT_SESSION_ID)?,
        worktree_path: get_opt_str(H_WORKTREE_PATH)?,
        worktree_branch: get_opt_str(H_WORKTREE_BRANCH)?,
    })
}

/// Read all messages from the Automerge document.
///
/// Returns messages in insertion order (Automerge map iteration order).
pub fn read_messages(doc: &AutoCommit) -> Result<Vec<MessageEntry>> {
    let messages_obj = doc
        .get(automerge::ROOT, KEY_MESSAGES)
        .map_err(session_err)?
        .and_then(|(val, id)| if matches!(val, Value::Object(ObjType::Map)) { Some(id) } else { None })
        .ok_or_else(|| session_err("messages map not found in document"))?;

    let mut entries = Vec::new();
    let keys: Vec<String> = doc.keys(&messages_obj).collect();

    for key in &keys {
        let msg_obj = doc
            .get(&messages_obj, key.as_str())
            .map_err(session_err)?
            .and_then(|(val, id)| if matches!(val, Value::Object(ObjType::Map)) { Some(id) } else { None })
            .ok_or_else(|| session_err(format!("message '{}' is not a map", key)))?;

        let parent_id = match doc.get(&msg_obj, M_PARENT_ID).map_err(session_err)? {
            Some((Value::Scalar(s), _)) => s.to_str().map(MessageId::new),
            _ => None,
        };

        let message_json = doc
            .get(&msg_obj, M_MESSAGE_JSON)
            .map_err(session_err)?
            .and_then(|(val, _)| if let Value::Scalar(s) = val { s.to_str().map(String::from) } else { None })
            .ok_or_else(|| session_err(format!("message '{}' has no message_json", key)))?;

        let message = serde_json::from_str(&message_json).map_err(session_err)?;

        let timestamp_str = doc
            .get(&msg_obj, M_TIMESTAMP)
            .map_err(session_err)?
            .and_then(|(val, _)| if let Value::Scalar(s) = val { s.to_str().map(String::from) } else { None })
            .ok_or_else(|| session_err(format!("message '{}' has no timestamp", key)))?;

        let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp_str)
            .map_err(session_err)?
            .with_timezone(&chrono::Utc);

        entries.push(MessageEntry {
            id: MessageId::new(key),
            parent_id,
            message,
            timestamp,
        });
    }

    Ok(entries)
}

/// Read all annotations from the Automerge document.
pub fn read_annotations(doc: &AutoCommit) -> Result<Vec<AnnotationEntry>> {
    let annotations_obj = doc
        .get(automerge::ROOT, KEY_ANNOTATIONS)
        .map_err(session_err)?
        .and_then(|(val, id)| if matches!(val, Value::Object(ObjType::List)) { Some(id) } else { None })
        .ok_or_else(|| session_err("annotations list not found in document"))?;

    let len = doc.length(&annotations_obj);
    let mut annotations = Vec::with_capacity(len);

    for i in 0..len {
        let ann_obj = doc
            .get(&annotations_obj, i)
            .map_err(session_err)?
            .and_then(|(val, id)| if matches!(val, Value::Object(ObjType::Map)) { Some(id) } else { None })
            .ok_or_else(|| session_err(format!("annotation at index {} is not a map", i)))?;

        let json = doc
            .get(&ann_obj, "data")
            .map_err(session_err)?
            .and_then(|(val, _)| if let Value::Scalar(s) = val { s.to_str().map(String::from) } else { None })
            .ok_or_else(|| session_err(format!("annotation at index {} has no data", i)))?;

        let annotation: AnnotationEntry = serde_json::from_str(&json).map_err(session_err)?;
        annotations.push(annotation);
    }

    Ok(annotations)
}

/// Reconstruct `Vec<SessionEntry>` from an Automerge document.
///
/// Produces: one Header, then all Messages (insertion order), then annotations
/// converted to their original SessionEntry variants. This output is compatible
/// with `SessionTree::build()`.
pub fn to_session_entries(doc: &AutoCommit) -> Result<Vec<SessionEntry>> {
    let header = read_header(doc)?;
    let messages = read_messages(doc)?;
    let annotations = read_annotations(doc)?;

    let mut entries = Vec::with_capacity(1 + messages.len() + annotations.len());
    entries.push(SessionEntry::Header(header));

    for msg in messages {
        entries.push(SessionEntry::Message(msg));
    }

    for ann in annotations {
        entries.push(ann.into_session_entry());
    }

    Ok(entries)
}

/// Save the full Automerge document to disk (compacted).
pub fn save_document(doc: &mut AutoCommit, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(session_err)?;
    }
    let bytes = doc.save();
    std::fs::write(path, bytes).map_err(session_err)?;
    Ok(())
}

/// Load an Automerge document from disk.
pub fn load_document(path: &Path) -> Result<AutoCommit> {
    let bytes = std::fs::read(path).map_err(session_err)?;
    AutoCommit::load(&bytes).map_err(session_err)
}

/// Save incremental changes only (fast append for ongoing writes).
///
/// Appends the incremental bytes to the file. The file must already contain
/// a full save from `save_document`. On next load, `AutoCommit::load()` reads
/// the full save and any appended incremental chunks.
pub fn save_incremental(doc: &mut AutoCommit, path: &Path) -> Result<()> {
    let bytes = doc.save_incremental();
    if bytes.is_empty() {
        return Ok(());
    }

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().append(true).open(path).map_err(session_err)?;
    file.write_all(&bytes).map_err(session_err)?;
    Ok(())
}

// --- AnnotationEntry ---

/// Flattened annotation type for the Automerge annotations list.
///
/// Covers all non-message, non-header session entry types. Each variant
/// is serialized as JSON into the Automerge document and tagged with a
/// `kind` discriminator for O(1) dispatch on read.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "ann_type")]
pub enum AnnotationEntry {
    Label(LabelEntry),
    Compaction(CompactionEntry),
    ModelChange(ModelChangeEntry),
    Branch(BranchEntry),
    Resume(ResumeEntry),
    Custom(CustomEntry),
}

impl AnnotationEntry {
    /// String tag for the annotation kind (stored in the Automerge map).
    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::Label(_) => "label",
            Self::Compaction(_) => "compaction",
            Self::ModelChange(_) => "model_change",
            Self::Branch(_) => "branch",
            Self::Resume(_) => "resume",
            Self::Custom(_) => "custom",
        }
    }

    /// Convert back to the original `SessionEntry` variant.
    pub fn into_session_entry(self) -> SessionEntry {
        match self {
            Self::Label(l) => SessionEntry::Label(l),
            Self::Compaction(c) => SessionEntry::Compaction(c),
            Self::ModelChange(m) => SessionEntry::ModelChange(m),
            Self::Branch(b) => SessionEntry::Branch(b),
            Self::Resume(r) => SessionEntry::Resume(r),
            Self::Custom(c) => SessionEntry::Custom(c),
        }
    }

    /// Create from a `SessionEntry` if it's an annotation type.
    /// Returns `None` for Header and Message entries.
    pub fn from_session_entry(entry: &SessionEntry) -> Option<Self> {
        match entry {
            SessionEntry::Label(l) => Some(Self::Label(l.clone())),
            SessionEntry::Compaction(c) => Some(Self::Compaction(c.clone())),
            SessionEntry::ModelChange(m) => Some(Self::ModelChange(m.clone())),
            SessionEntry::Branch(b) => Some(Self::Branch(b.clone())),
            SessionEntry::Resume(r) => Some(Self::Resume(r.clone())),
            SessionEntry::Custom(c) => Some(Self::Custom(c.clone())),
            SessionEntry::Header(_) | SessionEntry::Message(_) => None,
        }
    }
}

/// Migrate a JSONL session file to Automerge format.
///
/// Reads the JSONL file, builds an Automerge document, saves it with
/// `.automerge` extension alongside the original, and renames the
/// original to `.jsonl.bak`.
///
/// Returns the path to the new `.automerge` file.
///
/// Skips (returns Ok) if an `.automerge` file already exists for this session.
pub fn migrate_jsonl_to_automerge(jsonl_path: &Path) -> Result<MigrateResult> {
    let automerge_path = jsonl_path.with_extension("automerge");
    if automerge_path.exists() {
        return Ok(MigrateResult::Skipped);
    }

    let entries = crate::store::read_entries(jsonl_path)?;

    let header = entries
        .iter()
        .find_map(|e| {
            if let SessionEntry::Header(h) = e {
                Some(h.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| session_err("JSONL file has no header entry"))?;

    let mut doc = create_document(&header)?;

    for entry in &entries {
        match entry {
            SessionEntry::Message(m) => {
                put_message(&mut doc, m)?;
            }
            SessionEntry::Header(_) => {} // already handled
            other => {
                if let Some(annotation) = AnnotationEntry::from_session_entry(other) {
                    put_annotation(&mut doc, &annotation)?;
                }
            }
        }
    }

    save_document(&mut doc, &automerge_path)?;

    // Rename original to .jsonl.bak
    let backup_path = jsonl_path.with_extension("jsonl.bak");
    std::fs::rename(jsonl_path, &backup_path).map_err(session_err)?;

    let message_count = entries.iter().filter(|e| matches!(e, SessionEntry::Message(_))).count();

    Ok(MigrateResult::Migrated {
        path: automerge_path,
        message_count,
    })
}

/// Result of a migration operation.
#[derive(Debug)]
pub enum MigrateResult {
    /// File was already migrated (`.automerge` exists).
    Skipped,
    /// Successfully migrated.
    Migrated {
        path: std::path::PathBuf,
        message_count: usize,
    },
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use clankers_message::AgentMessage;
    use clankers_message::Content;
    use clankers_message::UserMessage;

    use super::*;

    fn test_header() -> HeaderEntry {
        HeaderEntry {
            session_id: "test-session-123".to_string(),
            created_at: Utc::now(),
            cwd: "/home/user/project".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            version: "0.1.0".to_string(),
            agent: Some("worker".to_string()),
            parent_session_id: None,
            worktree_path: Some("/tmp/worktree".to_string()),
            worktree_branch: Some("feature-branch".to_string()),
        }
    }

    fn test_message(id: &str, parent: Option<&str>, text: &str) -> MessageEntry {
        let msg_id = MessageId::new(id);
        MessageEntry {
            id: msg_id.clone(),
            parent_id: parent.map(MessageId::new),
            message: AgentMessage::User(UserMessage {
                id: msg_id,
                content: vec![Content::Text {
                    text: text.to_string(),
                }],
                timestamp: Utc::now(),
            }),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_create_document_and_read_header() {
        let header = test_header();
        let doc = create_document(&header).unwrap();
        let read_back = read_header(&doc).unwrap();

        assert_eq!(read_back.session_id, header.session_id);
        assert_eq!(read_back.cwd, header.cwd);
        assert_eq!(read_back.model, header.model);
        assert_eq!(read_back.version, header.version);
        assert_eq!(read_back.agent, header.agent);
        assert_eq!(read_back.parent_session_id, header.parent_session_id);
        assert_eq!(read_back.worktree_path, header.worktree_path);
        assert_eq!(read_back.worktree_branch, header.worktree_branch);
    }

    #[test]
    fn test_create_document_header_no_optionals() {
        let header = HeaderEntry {
            session_id: "minimal".to_string(),
            created_at: Utc::now(),
            cwd: "/tmp".to_string(),
            model: "test".to_string(),
            version: "1.0".to_string(),
            agent: None,
            parent_session_id: None,
            worktree_path: None,
            worktree_branch: None,
        };
        let doc = create_document(&header).unwrap();
        let read_back = read_header(&doc).unwrap();

        assert_eq!(read_back.session_id, "minimal");
        assert!(read_back.agent.is_none());
        assert!(read_back.worktree_path.is_none());
    }

    #[test]
    fn test_put_message_and_read_back() {
        let header = test_header();
        let mut doc = create_document(&header).unwrap();

        let msg1 = test_message("msg-1", None, "Hello world");
        let msg2 = test_message("msg-2", Some("msg-1"), "Response");

        put_message(&mut doc, &msg1).unwrap();
        put_message(&mut doc, &msg2).unwrap();

        let messages = read_messages(&doc).unwrap();
        assert_eq!(messages.len(), 2);

        let m1 = messages.iter().find(|m| m.id.0 == "msg-1").unwrap();
        assert!(m1.parent_id.is_none());
        if let AgentMessage::User(u) = &m1.message {
            if let Content::Text { text } = &u.content[0] {
                assert_eq!(text, "Hello world");
            } else {
                panic!("expected text content");
            }
        } else {
            panic!("expected user message");
        }

        let m2 = messages.iter().find(|m| m.id.0 == "msg-2").unwrap();
        assert_eq!(m2.parent_id.as_ref().unwrap().0, "msg-1");
    }

    #[test]
    fn test_put_message_preserves_parent_chain() {
        let header = test_header();
        let mut doc = create_document(&header).unwrap();

        let msg1 = test_message("a", None, "root");
        let msg2 = test_message("b", Some("a"), "child");
        let msg3 = test_message("c", Some("b"), "grandchild");

        put_message(&mut doc, &msg1).unwrap();
        put_message(&mut doc, &msg2).unwrap();
        put_message(&mut doc, &msg3).unwrap();

        let messages = read_messages(&doc).unwrap();
        let mc = messages.iter().find(|m| m.id.0 == "c").unwrap();
        assert_eq!(mc.parent_id.as_ref().unwrap().0, "b");
    }

    #[test]
    fn test_put_annotation_label() {
        let header = test_header();
        let mut doc = create_document(&header).unwrap();

        let label = AnnotationEntry::Label(LabelEntry {
            id: MessageId::new("lbl-1"),
            target_message_id: MessageId::new("msg-1"),
            label: "important".to_string(),
            timestamp: Utc::now(),
        });
        put_annotation(&mut doc, &label).unwrap();

        let annotations = read_annotations(&doc).unwrap();
        assert_eq!(annotations.len(), 1);
        match &annotations[0] {
            AnnotationEntry::Label(l) => {
                assert_eq!(l.label, "important");
                assert_eq!(l.target_message_id.0, "msg-1");
            }
            other => panic!("expected Label, got {:?}", other),
        }
    }

    #[test]
    fn test_put_annotation_compaction() {
        let header = test_header();
        let mut doc = create_document(&header).unwrap();

        let compaction = AnnotationEntry::Compaction(CompactionEntry {
            id: MessageId::new("cmp-1"),
            compacted_range: vec![MessageId::new("m1"), MessageId::new("m2")],
            summary: "summarized 2 messages".to_string(),
            tokens_before: 1000,
            tokens_after: 100,
            timestamp: Utc::now(),
        });
        put_annotation(&mut doc, &compaction).unwrap();

        let annotations = read_annotations(&doc).unwrap();
        assert_eq!(annotations.len(), 1);
        match &annotations[0] {
            AnnotationEntry::Compaction(c) => {
                assert_eq!(c.tokens_before, 1000);
                assert_eq!(c.tokens_after, 100);
                assert_eq!(c.compacted_range.len(), 2);
            }
            other => panic!("expected Compaction, got {:?}", other),
        }
    }

    #[test]
    fn test_put_annotation_model_change() {
        let header = test_header();
        let mut doc = create_document(&header).unwrap();

        let mc = AnnotationEntry::ModelChange(ModelChangeEntry {
            id: MessageId::new("mc-1"),
            from_model: "haiku".to_string(),
            to_model: "sonnet".to_string(),
            reason: "user_request".to_string(),
            timestamp: Utc::now(),
        });
        put_annotation(&mut doc, &mc).unwrap();

        let annotations = read_annotations(&doc).unwrap();
        assert_eq!(annotations.len(), 1);
        match &annotations[0] {
            AnnotationEntry::ModelChange(m) => {
                assert_eq!(m.from_model, "haiku");
                assert_eq!(m.to_model, "sonnet");
            }
            other => panic!("expected ModelChange, got {:?}", other),
        }
    }

    #[test]
    fn test_put_annotation_branch() {
        let header = test_header();
        let mut doc = create_document(&header).unwrap();

        let branch = AnnotationEntry::Branch(BranchEntry {
            id: MessageId::new("br-1"),
            from_message_id: MessageId::new("msg-3"),
            reason: "try alternate approach".to_string(),
            timestamp: Utc::now(),
        });
        put_annotation(&mut doc, &branch).unwrap();

        let annotations = read_annotations(&doc).unwrap();
        assert_eq!(annotations.len(), 1);
        match &annotations[0] {
            AnnotationEntry::Branch(b) => {
                assert_eq!(b.reason, "try alternate approach");
                assert_eq!(b.from_message_id.0, "msg-3");
            }
            other => panic!("expected Branch, got {:?}", other),
        }
    }

    #[test]
    fn test_multiple_annotations_order() {
        let header = test_header();
        let mut doc = create_document(&header).unwrap();

        let a1 = AnnotationEntry::Label(LabelEntry {
            id: MessageId::new("l1"),
            target_message_id: MessageId::new("m1"),
            label: "first".to_string(),
            timestamp: Utc::now(),
        });
        let a2 = AnnotationEntry::Branch(BranchEntry {
            id: MessageId::new("b1"),
            from_message_id: MessageId::new("m2"),
            reason: "branch".to_string(),
            timestamp: Utc::now(),
        });
        let a3 = AnnotationEntry::Label(LabelEntry {
            id: MessageId::new("l2"),
            target_message_id: MessageId::new("m3"),
            label: "third".to_string(),
            timestamp: Utc::now(),
        });

        put_annotation(&mut doc, &a1).unwrap();
        put_annotation(&mut doc, &a2).unwrap();
        put_annotation(&mut doc, &a3).unwrap();

        let annotations = read_annotations(&doc).unwrap();
        assert_eq!(annotations.len(), 3);
        assert_eq!(annotations[0].kind_str(), "label");
        assert_eq!(annotations[1].kind_str(), "branch");
        assert_eq!(annotations[2].kind_str(), "label");
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.automerge");

        let header = test_header();
        let mut doc = create_document(&header).unwrap();

        let msg1 = test_message("msg-1", None, "Hello");
        let msg2 = test_message("msg-2", Some("msg-1"), "World");
        put_message(&mut doc, &msg1).unwrap();
        put_message(&mut doc, &msg2).unwrap();

        let label = AnnotationEntry::Label(LabelEntry {
            id: MessageId::new("lbl-1"),
            target_message_id: MessageId::new("msg-1"),
            label: "checkpoint".to_string(),
            timestamp: Utc::now(),
        });
        put_annotation(&mut doc, &label).unwrap();

        save_document(&mut doc, &path).unwrap();

        let loaded = load_document(&path).unwrap();
        let h = read_header(&loaded).unwrap();
        assert_eq!(h.session_id, "test-session-123");

        let msgs = read_messages(&loaded).unwrap();
        assert_eq!(msgs.len(), 2);

        let anns = read_annotations(&loaded).unwrap();
        assert_eq!(anns.len(), 1);
    }

    #[test]
    fn test_save_incremental_produces_loadable_document() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("incremental.automerge");

        let header = test_header();
        let mut doc = create_document(&header).unwrap();
        let msg1 = test_message("msg-1", None, "First");
        put_message(&mut doc, &msg1).unwrap();

        // Full save first
        save_document(&mut doc, &path).unwrap();

        // Add more data, save incrementally
        let msg2 = test_message("msg-2", Some("msg-1"), "Second");
        put_message(&mut doc, &msg2).unwrap();
        save_incremental(&mut doc, &path).unwrap();

        let msg3 = test_message("msg-3", Some("msg-2"), "Third");
        put_message(&mut doc, &msg3).unwrap();
        save_incremental(&mut doc, &path).unwrap();

        // Load and verify all data present
        let loaded = load_document(&path).unwrap();
        let msgs = read_messages(&loaded).unwrap();
        assert_eq!(msgs.len(), 3);

        let m3 = msgs.iter().find(|m| m.id.0 == "msg-3").unwrap();
        assert_eq!(m3.parent_id.as_ref().unwrap().0, "msg-2");
    }

    #[test]
    fn test_to_session_entries_builds_valid_tree() {
        use crate::tree::SessionTree;

        let header = test_header();
        let mut doc = create_document(&header).unwrap();

        let msg1 = test_message("root", None, "Root message");
        let msg2a = test_message("branch-a", Some("root"), "Branch A");
        let msg2b = test_message("branch-b", Some("root"), "Branch B");
        let msg3 = test_message("branch-a-child", Some("branch-a"), "Branch A continued");

        put_message(&mut doc, &msg1).unwrap();
        put_message(&mut doc, &msg2a).unwrap();
        put_message(&mut doc, &msg2b).unwrap();
        put_message(&mut doc, &msg3).unwrap();

        let label = AnnotationEntry::Label(LabelEntry {
            id: MessageId::new("lbl-1"),
            target_message_id: MessageId::new("branch-a"),
            label: "important".to_string(),
            timestamp: Utc::now(),
        });
        put_annotation(&mut doc, &label).unwrap();

        let entries = to_session_entries(&doc).unwrap();

        // Header + 4 messages + 1 annotation = 6 entries
        assert_eq!(entries.len(), 6);
        assert!(matches!(&entries[0], SessionEntry::Header(_)));

        // Build tree and verify structure
        let tree = SessionTree::build(entries);
        assert_eq!(tree.message_count(), 4);

        // Walk branch A
        let branch_a = tree.walk_branch(&MessageId::new("branch-a-child"));
        assert_eq!(branch_a.len(), 3);
        assert_eq!(branch_a[0].id.0, "root");
        assert_eq!(branch_a[1].id.0, "branch-a");
        assert_eq!(branch_a[2].id.0, "branch-a-child");

        // Walk branch B
        let branch_b = tree.walk_branch(&MessageId::new("branch-b"));
        assert_eq!(branch_b.len(), 2);
        assert_eq!(branch_b[0].id.0, "root");
        assert_eq!(branch_b[1].id.0, "branch-b");

        // Branch point detection
        assert!(tree.is_branch_point(&MessageId::new("root")));
        assert!(!tree.is_branch_point(&MessageId::new("branch-a")));

        // Leaves
        let leaves = tree.find_all_leaves();
        assert_eq!(leaves.len(), 2);
        let leaf_ids: Vec<&str> = leaves.iter().map(|l| l.id.0.as_str()).collect();
        assert!(leaf_ids.contains(&"branch-a-child"));
        assert!(leaf_ids.contains(&"branch-b"));
    }

    #[test]
    fn test_annotation_entry_from_session_entry() {
        let label = SessionEntry::Label(LabelEntry {
            id: MessageId::new("l1"),
            target_message_id: MessageId::new("m1"),
            label: "test".to_string(),
            timestamp: Utc::now(),
        });
        assert!(AnnotationEntry::from_session_entry(&label).is_some());

        let header = SessionEntry::Header(test_header());
        assert!(AnnotationEntry::from_session_entry(&header).is_none());

        let msg = SessionEntry::Message(test_message("m1", None, "text"));
        assert!(AnnotationEntry::from_session_entry(&msg).is_none());
    }

    #[test]
    fn test_annotation_entry_roundtrip_via_session_entry() {
        let original = AnnotationEntry::ModelChange(ModelChangeEntry {
            id: MessageId::new("mc-1"),
            from_model: "haiku".to_string(),
            to_model: "sonnet".to_string(),
            reason: "cost".to_string(),
            timestamp: Utc::now(),
        });

        let session_entry = original.clone().into_session_entry();
        let recovered = AnnotationEntry::from_session_entry(&session_entry).unwrap();
        assert_eq!(recovered.kind_str(), "model_change");
    }

    #[test]
    fn test_save_creates_parent_dirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("deep").join("nested").join("session.automerge");

        let header = test_header();
        let mut doc = create_document(&header).unwrap();
        save_document(&mut doc, &path).unwrap();

        assert!(path.exists());
    }

    #[test]
    fn test_load_nonexistent_file() {
        let result = load_document(Path::new("/tmp/nonexistent-automerge-file.automerge"));
        assert!(result.is_err());
    }

    #[test]
    fn test_migrate_jsonl_to_automerge() {
        let tmp = tempfile::TempDir::new().unwrap();
        let jsonl_path = tmp.path().join("test_session.jsonl");

        // Write a JSONL session file
        let header = crate::entry::SessionEntry::Header(test_header());
        crate::store::append_entry(&jsonl_path, &header).unwrap();

        let msg = crate::entry::SessionEntry::Message(test_message("msg-1", None, "Hello"));
        crate::store::append_entry(&jsonl_path, &msg).unwrap();

        let msg2 = crate::entry::SessionEntry::Message(test_message("msg-2", Some("msg-1"), "World"));
        crate::store::append_entry(&jsonl_path, &msg2).unwrap();

        // Migrate
        let result = migrate_jsonl_to_automerge(&jsonl_path).unwrap();
        match result {
            MigrateResult::Migrated { path, message_count } => {
                assert!(path.exists());
                assert_eq!(message_count, 2);
                assert!(path.extension().unwrap() == "automerge");

                // Original renamed to .bak
                assert!(!jsonl_path.exists());
                assert!(jsonl_path.with_extension("jsonl.bak").exists());

                // Verify the migrated doc
                let doc = load_document(&path).unwrap();
                let h = read_header(&doc).unwrap();
                assert_eq!(h.session_id, "test-session-123");
                let msgs = read_messages(&doc).unwrap();
                assert_eq!(msgs.len(), 2);
            }
            MigrateResult::Skipped => panic!("expected migration, got skip"),
        }
    }

    #[test]
    fn test_migrate_skips_already_migrated() {
        let tmp = tempfile::TempDir::new().unwrap();
        let jsonl_path = tmp.path().join("test_session.jsonl");
        let automerge_path = tmp.path().join("test_session.automerge");

        // Write both files
        let header = crate::entry::SessionEntry::Header(test_header());
        crate::store::append_entry(&jsonl_path, &header).unwrap();
        std::fs::write(&automerge_path, b"existing").unwrap();

        let result = migrate_jsonl_to_automerge(&jsonl_path).unwrap();
        assert!(matches!(result, MigrateResult::Skipped));

        // Original .jsonl still exists (not renamed)
        assert!(jsonl_path.exists());
    }

    #[test]
    fn test_migrate_preserves_tree_structure() {
        use crate::tree::SessionTree;

        let tmp = tempfile::TempDir::new().unwrap();
        let jsonl_path = tmp.path().join("branched.jsonl");

        // Build a JSONL session with branches
        let header = crate::entry::SessionEntry::Header(test_header());
        crate::store::append_entry(&jsonl_path, &header).unwrap();

        let root = test_message("root", None, "Root");
        let a1 = test_message("a1", Some("root"), "Branch A");
        let b1 = test_message("b1", Some("root"), "Branch B");
        let a2 = test_message("a2", Some("a1"), "Branch A continued");

        for msg in [root, a1, b1, a2] {
            crate::store::append_entry(&jsonl_path, &crate::entry::SessionEntry::Message(msg)).unwrap();
        }

        // Add a label annotation via JSONL
        let label = crate::entry::SessionEntry::Label(crate::entry::LabelEntry {
            id: MessageId::new("lbl-1"),
            target_message_id: MessageId::new("a1"),
            label: "checkpoint".to_string(),
            timestamp: Utc::now(),
        });
        crate::store::append_entry(&jsonl_path, &label).unwrap();

        // Migrate
        let result = migrate_jsonl_to_automerge(&jsonl_path).unwrap();
        let path = match result {
            MigrateResult::Migrated { path, .. } => path,
            MigrateResult::Skipped => panic!("expected migration"),
        };

        // Verify tree structure in migrated doc
        let doc = load_document(&path).unwrap();
        let entries = to_session_entries(&doc).unwrap();
        let tree = SessionTree::build(entries.clone());

        assert_eq!(tree.message_count(), 4);
        assert!(tree.is_branch_point(&MessageId::new("root")));

        let branch_a = tree.walk_branch(&MessageId::new("a2"));
        assert_eq!(branch_a.len(), 3);

        let branch_b = tree.walk_branch(&MessageId::new("b1"));
        assert_eq!(branch_b.len(), 2);

        // Label annotation preserved
        let has_label = entries.iter().any(|e| {
            matches!(e, crate::entry::SessionEntry::Label(l) if l.label == "checkpoint")
        });
        assert!(has_label);
    }
}
