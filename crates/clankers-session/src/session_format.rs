use std::path::Path;
use std::path::PathBuf;

use automerge::AutoCommit;
use clanker_message::Content;
use clanker_message::transcript::AgentMessage;

use crate::automerge_store;
use crate::automerge_store::AnnotationEntry;
use crate::entry::HeaderEntry;
use crate::entry::SessionEntry;
use crate::error::Result;
use crate::error::SessionError;
use crate::store;
use crate::store::SessionSummary;

pub(crate) struct OpenedSessionDocument {
    pub(crate) doc: AutoCommit,
    pub(crate) file_path: PathBuf,
}

trait SessionFormat {
    fn load_entries(&self, path: &Path) -> Result<Vec<SessionEntry>>;
    fn open_as_automerge(&self, file_path: PathBuf) -> Result<OpenedSessionDocument>;
    fn read_summary(&self, path: &Path) -> Option<SessionSummary>;
    fn import_destination(&self, sessions_dir: &Path, source: &Path) -> Result<PathBuf>;
}

struct AutomergeSessionFormat;
struct JsonlSessionFormat;

static AUTOMERGE_FORMAT: AutomergeSessionFormat = AutomergeSessionFormat;
static JSONL_FORMAT: JsonlSessionFormat = JsonlSessionFormat;

const MESSAGE_PREVIEW_MAX_BYTES: usize = 80;
const MESSAGE_PREVIEW_SUFFIX_BYTES: usize = "…".len();

fn format_for_path(path: &Path) -> &'static dyn SessionFormat {
    if path.extension().is_some_and(|ext| ext == "automerge") {
        &AUTOMERGE_FORMAT
    } else {
        &JSONL_FORMAT
    }
}

pub(crate) fn load_entries(path: &Path) -> Result<Vec<SessionEntry>> {
    format_for_path(path).load_entries(path)
}

pub(crate) fn open_as_automerge(file_path: PathBuf) -> Result<OpenedSessionDocument> {
    format_for_path(&file_path).open_as_automerge(file_path)
}

pub(crate) fn read_summary(path: &Path) -> Option<SessionSummary> {
    format_for_path(path).read_summary(path)
}

pub(crate) fn import_destination(sessions_dir: &Path, source: &Path) -> Result<PathBuf> {
    format_for_path(source).import_destination(sessions_dir, source)
}

impl SessionFormat for AutomergeSessionFormat {
    fn load_entries(&self, path: &Path) -> Result<Vec<SessionEntry>> {
        let doc = automerge_store::load_document(path)?;
        automerge_store::to_session_entries(&doc)
    }

    fn open_as_automerge(&self, file_path: PathBuf) -> Result<OpenedSessionDocument> {
        let doc = automerge_store::load_document(&file_path)?;
        Ok(OpenedSessionDocument { doc, file_path })
    }

    fn read_summary(&self, path: &Path) -> Option<SessionSummary> {
        let doc = automerge_store::load_document(path).ok()?;
        let header = automerge_store::read_header(&doc).ok()?;
        let messages = automerge_store::read_messages(&doc).ok()?;
        summary_from_header_and_messages(header, messages.into_iter(), path)
    }

    fn import_destination(&self, sessions_dir: &Path, source: &Path) -> Result<PathBuf> {
        let doc = automerge_store::load_document(source)?;
        let header = automerge_store::read_header(&doc)?;
        Ok(store::session_file_path_automerge_at(
            store::SessionFilePathRequest {
                sessions_dir,
                cwd: &header.cwd,
                session_id: &header.session_id,
            },
            header.created_at,
        ))
    }
}

impl SessionFormat for JsonlSessionFormat {
    fn load_entries(&self, path: &Path) -> Result<Vec<SessionEntry>> {
        store::read_entries(path)
    }

    fn open_as_automerge(&self, file_path: PathBuf) -> Result<OpenedSessionDocument> {
        let entries = self.load_entries(&file_path)?;
        let mut doc = build_automerge_doc_from_entries(&entries)?;
        let automerge_path = file_path.with_extension("automerge");
        automerge_store::save_document(&mut doc, &automerge_path)?;
        Ok(OpenedSessionDocument {
            doc,
            file_path: automerge_path,
        })
    }

    fn read_summary(&self, path: &Path) -> Option<SessionSummary> {
        let entries = store::read_entries(path).ok()?;
        let header = entries.iter().find_map(header_entry)?.clone();
        let messages = entries.into_iter().filter_map(message_entry);
        summary_from_header_and_messages(header, messages, path)
    }

    fn import_destination(&self, sessions_dir: &Path, source: &Path) -> Result<PathBuf> {
        let entries = store::read_entries(source)?;
        let header = entries
            .into_iter()
            .find_map(|entry| {
                if let SessionEntry::Header(header) = entry {
                    Some(header)
                } else {
                    None
                }
            })
            .ok_or_else(|| SessionError {
                message: "Import file has no header entry".into(),
            })?;
        Ok(store::session_file_path_at(
            store::SessionFilePathRequest {
                sessions_dir,
                cwd: &header.cwd,
                session_id: &header.session_id,
            },
            header.created_at,
        ))
    }
}

fn build_automerge_doc_from_entries(entries: &[SessionEntry]) -> Result<AutoCommit> {
    assert!(!entries.is_empty(), "Automerge import requires at least one session entry");
    assert!(entries.iter().any(|entry| matches!(entry, SessionEntry::Header(_))), "Automerge import requires a header entry");

    let header = entries.iter().find_map(header_entry).cloned().ok_or_else(|| SessionError {
        message: "No header entry".into(),
    })?;

    let mut doc = automerge_store::create_document(&header)?;

    for entry in entries {
        match entry {
            SessionEntry::Message(message) => {
                automerge_store::put_message(&mut doc, message)?;
            }
            SessionEntry::Header(_) => {}
            other => {
                if let Some(annotation) = AnnotationEntry::from_session_entry(other) {
                    automerge_store::put_annotation(&mut doc, &annotation)?;
                }
            }
        }
    }

    Ok(doc)
}

fn header_entry(entry: &SessionEntry) -> Option<&HeaderEntry> {
    if let SessionEntry::Header(header) = entry {
        Some(header)
    } else {
        None
    }
}

fn message_entry(entry: SessionEntry) -> Option<crate::entry::MessageEntry> {
    if let SessionEntry::Message(message) = entry {
        Some(message)
    } else {
        None
    }
}

fn summary_from_header_and_messages(
    header: HeaderEntry,
    messages: impl Iterator<Item = crate::entry::MessageEntry>,
    path: &Path,
) -> Option<SessionSummary> {
    let mut message_count = 0usize;
    let mut first_user_message = None;

    for message in messages {
        message_count += 1;
        if first_user_message.is_none() {
            first_user_message = first_user_preview(&message.message);
        }
    }

    Some(SessionSummary {
        session_id: header.session_id,
        cwd: header.cwd,
        model: header.model,
        created_at: header.created_at,
        message_count,
        first_user_message,
        file_path: path.to_path_buf(),
    })
}

fn first_user_preview(message: &AgentMessage) -> Option<String> {
    assert!(MESSAGE_PREVIEW_MAX_BYTES > 0, "message preview budget must be non-zero");
    assert!(MESSAGE_PREVIEW_SUFFIX_BYTES > 0, "message preview suffix must be non-empty");

    let AgentMessage::User(user) = message else {
        return None;
    };
    let text: String = user
        .content
        .iter()
        .filter_map(|content| {
            if let Content::Text { text } = content {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if text.is_empty() {
        None
    } else if text.len() > MESSAGE_PREVIEW_MAX_BYTES {
        Some(format!("{}…", &text[..MESSAGE_PREVIEW_MAX_BYTES]))
    } else {
        Some(text)
    }
}
