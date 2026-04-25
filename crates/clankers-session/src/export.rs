//! Session export to markdown, plain text, and JSON formats.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::fmt::Write;
use std::path::Path;

use clanker_message::AgentMessage;
use clanker_message::Content;

use super::entry::SessionEntry;
use crate::error::Result;
use crate::error::SessionError;

/// Read session entries from a file, supporting both `.automerge` and `.jsonl` formats.
fn load_entries(path: &Path) -> Result<Vec<SessionEntry>> {
    if path.extension().is_some_and(|ext| ext == "automerge") {
        let doc = crate::automerge_store::load_document(path)?;
        crate::automerge_store::to_session_entries(&doc)
    } else {
        crate::store::read_entries(path)
    }
}

/// Export a session to markdown format
pub fn export_markdown(path: &Path) -> Result<String> {
    let entries = load_entries(path)?;
    let mut out = String::new();

    for entry in &entries {
        match entry {
            SessionEntry::Header(h) => {
                write!(out, "# Session: {}\n\n", h.session_id).ok();
                writeln!(out, "- **Date**: {}", h.created_at.format("%Y-%m-%d %H:%M:%S UTC")).ok();
                writeln!(out, "- **Model**: {}", h.model).ok();
                write!(out, "- **CWD**: {}\n\n---\n\n", h.cwd).ok();
            }
            SessionEntry::Message(m) => match &m.message {
                AgentMessage::User(u) => {
                    out.push_str("## 🧑 User\n\n");
                    for c in &u.content {
                        if let Content::Text { text } = c {
                            out.push_str(text);
                            out.push_str("\n\n");
                        }
                    }
                }
                AgentMessage::Assistant(a) => {
                    out.push_str("## 🤖 Assistant\n\n");
                    for c in &a.content {
                        match c {
                            Content::Text { text } => {
                                out.push_str(text);
                                out.push_str("\n\n");
                            }
                            Content::ToolUse { name, input, .. } => {
                                write!(
                                    out,
                                    "**Tool call**: `{}`\n```json\n{}\n```\n\n",
                                    name,
                                    serde_json::to_string_pretty(input).unwrap_or_default()
                                )
                                .ok();
                            }
                            Content::Thinking { thinking, .. } => {
                                write!(
                                    out,
                                    "<details>\n<summary>💭 Thinking</summary>\n\n{}\n\n</details>\n\n",
                                    thinking
                                )
                                .ok();
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
                    write!(out, "### {} ({})\n\n", label, tr.tool_name).ok();
                    for c in &tr.content {
                        if let Content::Text { text } = c {
                            write!(out, "```\n{}\n```\n\n", text).ok();
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    Ok(out)
}

/// Export a session to plain text format
pub fn export_text(path: &Path) -> Result<String> {
    let entries = load_entries(path)?;
    let mut out = String::new();

    for entry in &entries {
        match entry {
            SessionEntry::Header(h) => {
                writeln!(
                    out,
                    "Session: {} | Model: {} | {}",
                    h.session_id,
                    h.model,
                    h.created_at.format("%Y-%m-%d %H:%M")
                )
                .ok();
                writeln!(out, "CWD: {}", h.cwd).ok();
                out.push_str(&"─".repeat(60));
                out.push('\n');
            }
            SessionEntry::Message(m) => match &m.message {
                AgentMessage::User(u) => {
                    out.push_str("\n[User]\n");
                    for c in &u.content {
                        if let Content::Text { text } = c {
                            out.push_str(text);
                            out.push('\n');
                        }
                    }
                }
                AgentMessage::Assistant(a) => {
                    out.push_str("\n[Assistant]\n");
                    for c in &a.content {
                        if let Content::Text { text } = c {
                            out.push_str(text);
                            out.push('\n');
                        }
                    }
                }
                AgentMessage::ToolResult(tr) => {
                    let label = if tr.is_error { "Tool Error" } else { "Tool Result" };
                    write!(out, "\n[{} - {}]\n", label, tr.tool_name).ok();
                    for c in &tr.content {
                        if let Content::Text { text } = c {
                            out.push_str(text);
                            out.push('\n');
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    Ok(out)
}

/// Export a session to JSONL format (one entry per line).
///
/// For `.automerge` files, reads the document and serializes entries as JSONL.
/// For `.jsonl` files, re-serializes (normalizes) entries.
pub fn export_jsonl(path: &Path) -> Result<String> {
    let entries = load_entries(path)?;
    let mut out = String::new();
    for entry in &entries {
        let line = serde_json::to_string(entry).map_err(|e| SessionError {
            message: format!("JSONL serialization failed: {}", e),
        })?;
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}

/// Export a session to structured JSON format
pub fn export_json(path: &Path) -> Result<String> {
    let entries = load_entries(path)?;
    serde_json::to_string_pretty(&entries).map_err(|e| SessionError {
        message: format!("JSON serialization failed: {}", e),
    })
}

#[cfg(test)]
mod tests {
    use clanker_message::MessageId;
    use clanker_message::UserMessage;
    use tempfile::TempDir;

    use super::*;
    use crate::store::append_entry;

    fn make_header(session_id: &str) -> SessionEntry {
        SessionEntry::Header(crate::entry::HeaderEntry {
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
        SessionEntry::Message(crate::entry::MessageEntry {
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
