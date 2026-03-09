//! Session export to markdown, plain text, and JSON formats.

use std::path::Path;

use super::entry::SessionEntry;
use super::store::read_entries;
use crate::error::Result;

/// Export a session to markdown format
pub fn export_markdown(path: &Path) -> Result<String> {
    let entries = read_entries(path)?;
    let mut out = String::new();

    for entry in &entries {
        match entry {
            SessionEntry::Header(h) => {
                out.push_str(&format!("# Session: {}\n\n", h.session_id));
                out.push_str(&format!("- **Date**: {}\n", h.created_at.format("%Y-%m-%d %H:%M:%S UTC")));
                out.push_str(&format!("- **Model**: {}\n", h.model));
                out.push_str(&format!("- **CWD**: {}\n\n---\n\n", h.cwd));
            }
            SessionEntry::Message(m) => {
                use crate::provider::message::AgentMessage;
                match &m.message {
                    AgentMessage::User(u) => {
                        out.push_str("## 🧑 User\n\n");
                        for c in &u.content {
                            if let crate::provider::message::Content::Text { text } = c {
                                out.push_str(text);
                                out.push_str("\n\n");
                            }
                        }
                    }
                    AgentMessage::Assistant(a) => {
                        out.push_str("## 🤖 Assistant\n\n");
                        for c in &a.content {
                            match c {
                                crate::provider::message::Content::Text { text } => {
                                    out.push_str(text);
                                    out.push_str("\n\n");
                                }
                                crate::provider::message::Content::ToolUse { name, input, .. } => {
                                    out.push_str(&format!(
                                        "**Tool call**: `{}`\n```json\n{}\n```\n\n",
                                        name,
                                        serde_json::to_string_pretty(input).unwrap_or_default()
                                    ));
                                }
                                crate::provider::message::Content::Thinking { thinking } => {
                                    out.push_str(&format!(
                                        "<details>\n<summary>💭 Thinking</summary>\n\n{}\n\n</details>\n\n",
                                        thinking
                                    ));
                                }
                                _ => {}
                            }
                        }
                    }
                    AgentMessage::ToolResult(tr) => {
                        let label = if tr.is_error {
                            "❌ Tool Error"
                        } else {
                            "📋 Tool Result"
                        };
                        out.push_str(&format!("### {} ({})\n\n", label, tr.tool_name));
                        for c in &tr.content {
                            if let crate::provider::message::Content::Text { text } = c {
                                out.push_str(&format!("```\n{}\n```\n\n", text));
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Export a session to plain text format
pub fn export_text(path: &Path) -> Result<String> {
    let entries = read_entries(path)?;
    let mut out = String::new();

    for entry in &entries {
        match entry {
            SessionEntry::Header(h) => {
                out.push_str(&format!(
                    "Session: {} | Model: {} | {}\n",
                    h.session_id,
                    h.model,
                    h.created_at.format("%Y-%m-%d %H:%M")
                ));
                out.push_str(&format!("CWD: {}\n", h.cwd));
                out.push_str(&"─".repeat(60));
                out.push('\n');
            }
            SessionEntry::Message(m) => {
                use crate::provider::message::AgentMessage;
                match &m.message {
                    AgentMessage::User(u) => {
                        out.push_str("\n[User]\n");
                        for c in &u.content {
                            if let crate::provider::message::Content::Text { text } = c {
                                out.push_str(text);
                                out.push('\n');
                            }
                        }
                    }
                    AgentMessage::Assistant(a) => {
                        out.push_str("\n[Assistant]\n");
                        for c in &a.content {
                            if let crate::provider::message::Content::Text { text } = c {
                                out.push_str(text);
                                out.push('\n');
                            }
                        }
                    }
                    AgentMessage::ToolResult(tr) => {
                        let label = if tr.is_error { "Tool Error" } else { "Tool Result" };
                        out.push_str(&format!("\n[{} - {}]\n", label, tr.tool_name));
                        for c in &tr.content {
                            if let crate::provider::message::Content::Text { text } = c {
                                out.push_str(text);
                                out.push('\n');
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Export a session to structured JSON format
pub fn export_json(path: &Path) -> Result<String> {
    let entries = read_entries(path)?;
    serde_json::to_string_pretty(&entries).map_err(|e| crate::error::Error::Session {
        message: format!("JSON serialization failed: {}", e),
    })
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::provider::message::AgentMessage;
    use crate::provider::message::Content;
    use crate::provider::message::MessageId;
    use crate::provider::message::UserMessage;
    use crate::session::store::append_entry;

    fn make_header(session_id: &str) -> SessionEntry {
        SessionEntry::Header(crate::session::entry::HeaderEntry {
            session_id: session_id.to_string(),
            created_at: chrono::Utc::now(),
            cwd: "/tmp/test".to_string(),
            model: "test-model".to_string(),
            version: "1.0.0".to_string(),
            agent: None,
            parent_session_id: None,
            worktree_path: None,
            worktree_branch: None,
        })
    }

    fn make_message(id: MessageId, parent: Option<MessageId>) -> SessionEntry {
        SessionEntry::Message(crate::session::entry::MessageEntry {
            id: id.clone(),
            parent_id: parent,
            message: AgentMessage::User(UserMessage {
                id,
                content: vec![Content::Text {
                    text: "Test".to_string(),
                }],
                timestamp: chrono::Utc::now(),
            }),
            timestamp: chrono::Utc::now(),
        })
    }

    #[test]
    fn test_export_text() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let file = temp.path().join("test.jsonl");

        let header = make_header("exp1");
        append_entry(&file, &header).expect("failed to append header");
        let msg = make_message(MessageId::new("m1"), None);
        append_entry(&file, &msg).expect("failed to append message");

        let text = export_text(&file).expect("failed to export text");
        assert!(text.contains("Session: exp1"));
        assert!(text.contains("[User]"));
        assert!(text.contains("Test"));
    }

    #[test]
    fn test_export_markdown() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let file = temp.path().join("test.jsonl");

        let header = make_header("md1");
        append_entry(&file, &header).expect("failed to append header");
        let msg = make_message(MessageId::new("m1"), None);
        append_entry(&file, &msg).expect("failed to append message");

        let md = export_markdown(&file).expect("failed to export markdown");
        assert!(md.contains("# Session: md1"));
        assert!(md.contains("## 🧑 User"));
    }

    #[test]
    fn test_export_json() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let file = temp.path().join("test.jsonl");

        let header = make_header("json1");
        append_entry(&file, &header).expect("failed to append header");

        let json = export_json(&file).expect("failed to export json");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("failed to parse json");
        assert!(parsed.is_array());
    }
}
