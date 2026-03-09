//! JSONL append-only file I/O for sessions

use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use super::entry::SessionEntry;
use crate::error::Result;

/// Read all entries from a session JSONL file
pub fn read_entries(path: &Path) -> Result<Vec<SessionEntry>> {
    use snafu::ResultExt;
    let file = std::fs::File::open(path).context(crate::error::IoSnafu)?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line.context(crate::error::IoSnafu)?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: SessionEntry = serde_json::from_str(&line).context(crate::error::JsonSnafu)?;
        entries.push(entry);
    }
    Ok(entries)
}

/// Append an entry to a session JSONL file
pub fn append_entry(path: &Path, entry: &SessionEntry) -> Result<()> {
    use snafu::ResultExt;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context(crate::error::IoSnafu)?;
    }
    let json = serde_json::to_string(entry).context(crate::error::JsonSnafu)?;
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path).context(crate::error::IoSnafu)?;
    writeln!(file, "{}", json).context(crate::error::IoSnafu)?;
    Ok(())
}

/// Generate session file path
pub fn session_file_path(sessions_dir: &Path, cwd: &str, session_id: &str) -> PathBuf {
    // Encode cwd as safe directory name
    let encoded_cwd = encode_cwd(cwd);
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    sessions_dir.join(&encoded_cwd).join(format!("{}_{}.jsonl", timestamp, session_id))
}

/// Encode a cwd path into a safe directory name
fn encode_cwd(cwd: &str) -> String {
    cwd.replace(['/', '\\', ':'], "_")
}

/// List session files for a given cwd
pub fn list_sessions(sessions_dir: &Path, cwd: &str) -> Vec<PathBuf> {
    let encoded = encode_cwd(cwd);
    let dir = sessions_dir.join(&encoded);
    if !dir.is_dir() {
        return vec![];
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    files.sort();
    files.reverse(); // Most recent first
    files
}

/// List all session directories (all cwds)
pub fn list_all_sessions(sessions_dir: &Path) -> Vec<PathBuf> {
    if !sessions_dir.is_dir() {
        return vec![];
    }
    let mut all_files = Vec::new();
    if let Ok(dirs) = std::fs::read_dir(sessions_dir) {
        for dir_entry in dirs.flatten() {
            let dir_path = dir_entry.path();
            if dir_path.is_dir()
                && let Ok(files) = std::fs::read_dir(&dir_path)
            {
                for file_entry in files.flatten() {
                    let path = file_entry.path();
                    if path.extension().is_some_and(|ext| ext == "jsonl") {
                        all_files.push(path);
                    }
                }
            }
        }
    }
    all_files.sort();
    all_files.reverse();
    all_files
}

/// Delete all sessions for a given cwd
pub fn purge_sessions(sessions_dir: &Path, cwd: &str) -> std::io::Result<usize> {
    let files = list_sessions(sessions_dir, cwd);
    let count = files.len();
    for f in &files {
        std::fs::remove_file(f)?;
    }
    Ok(count)
}

/// Delete all sessions across all cwds
pub fn purge_all_sessions(sessions_dir: &Path) -> std::io::Result<usize> {
    let files = list_all_sessions(sessions_dir);
    let count = files.len();
    for f in &files {
        std::fs::remove_file(f)?;
    }
    Ok(count)
}

/// Summary metadata extracted from a session file
#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub session_id: String,
    pub cwd: String,
    pub model: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub message_count: usize,
    pub first_user_message: Option<String>,
    pub file_path: PathBuf,
}

/// Read session summary (header + first user message) without loading all entries
pub fn read_session_summary(path: &Path) -> Option<SessionSummary> {
    use std::io::BufRead;
    let file = std::fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);

    let mut header: Option<super::entry::HeaderEntry> = None;
    let mut first_user_text: Option<String> = None;
    let mut message_count: usize = 0;

    for line in reader.lines() {
        let line = line.ok()?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<SessionEntry>(&line) {
            match entry {
                SessionEntry::Header(h) => header = Some(h),
                SessionEntry::Message(m) => {
                    message_count += 1;
                    if first_user_text.is_none()
                        && let crate::provider::message::AgentMessage::User(ref u) = m.message
                    {
                        let text: String = u
                            .content
                            .iter()
                            .filter_map(|c| {
                                if let crate::provider::message::Content::Text { text } = c {
                                    Some(text.as_str())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                        if !text.is_empty() {
                            // Truncate to 80 chars for preview
                            let preview = if text.len() > 80 {
                                format!("{}…", &text[..80])
                            } else {
                                text
                            };
                            first_user_text = Some(preview);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let h = header?;
    Some(SessionSummary {
        session_id: h.session_id,
        cwd: h.cwd,
        model: h.model,
        created_at: h.created_at,
        message_count,
        first_user_message: first_user_text,
        file_path: path.to_path_buf(),
    })
}

/// Import a session from a JSONL file into the sessions directory
pub fn import_session(sessions_dir: &Path, source: &Path) -> Result<PathBuf> {
    let entries = read_entries(source)?;
    let header = entries
        .iter()
        .find_map(|e| if let SessionEntry::Header(h) = e { Some(h) } else { None })
        .ok_or_else(|| crate::error::Error::Session {
            message: "Import file has no header entry".into(),
        })?;

    let dest = session_file_path(sessions_dir, &header.cwd, &header.session_id);
    if dest.exists() {
        return Err(crate::error::Error::Session {
            message: format!("Session {} already exists at {}", header.session_id, dest.display()),
        });
    }

    // Copy the file
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| crate::error::Error::Session {
            message: format!("Failed to create directory: {}", e),
        })?;
    }
    std::fs::copy(source, &dest).map_err(|e| crate::error::Error::Session {
        message: format!("Failed to copy session file: {}", e),
    })?;

    Ok(dest)
}

/// Find a session file by partial ID match
pub fn find_session_by_id(sessions_dir: &Path, cwd: &str, partial_id: &str) -> Option<PathBuf> {
    list_sessions(sessions_dir, cwd)
        .into_iter()
        .find(|f| f.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.contains(partial_id)))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use tempfile::TempDir;

    use super::*;
    use crate::provider::message::AgentMessage;
    use crate::provider::message::Content;
    use crate::provider::message::MessageId;
    use crate::provider::message::UserMessage;

    fn make_header(session_id: &str) -> SessionEntry {
        SessionEntry::Header(crate::session::entry::HeaderEntry {
            session_id: session_id.to_string(),
            created_at: Utc::now(),
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
                id: id.clone(),
                content: vec![Content::Text {
                    text: "Test".to_string(),
                }],
                timestamp: Utc::now(),
            }),
            timestamp: Utc::now(),
        })
    }

    #[test]
    fn test_encode_cwd() {
        assert_eq!(encode_cwd("/home/user/project"), "_home_user_project");
        assert_eq!(encode_cwd("C:\\Users\\test"), "C__Users_test");
        assert_eq!(encode_cwd("/tmp/test:path"), "_tmp_test_path");
    }

    #[test]
    fn test_write_and_read_entries() -> Result<()> {
        let temp = TempDir::new().expect("test: failed to create temp dir");
        let session_file = temp.path().join("test_session.jsonl");

        // Write entries
        let header = make_header("sess1");
        let msg = make_message(MessageId::new("msg1"), None);

        append_entry(&session_file, &header)?;
        append_entry(&session_file, &msg)?;

        // Read back
        let entries = read_entries(&session_file)?;
        assert_eq!(entries.len(), 2);

        match &entries[0] {
            SessionEntry::Header(h) => assert_eq!(h.session_id, "sess1"),
            _ => panic!("Expected header"),
        }

        match &entries[1] {
            SessionEntry::Message(_) => {}
            _ => panic!("Expected message"),
        }

        Ok(())
    }

    #[test]
    fn test_read_nonexistent_file() {
        let temp = TempDir::new().expect("test: failed to create temp dir");
        let result = read_entries(&temp.path().join("nonexistent.jsonl"));
        assert!(result.is_err());
    }

    #[test]
    fn test_append_creates_parent_dirs() -> Result<()> {
        let temp = TempDir::new().expect("test: failed to create temp dir");
        let nested_path = temp.path().join("deep").join("nested").join("session.jsonl");

        let header = make_header("test");
        append_entry(&nested_path, &header)?;

        assert!(nested_path.exists());
        Ok(())
    }

    #[test]
    fn test_session_file_path_format() {
        let sessions_dir = Path::new("/tmp/sessions");
        let cwd = "/home/user/project";
        let session_id = "abc123";

        let path = session_file_path(sessions_dir, cwd, session_id);
        let path_str = path.to_string_lossy();

        assert!(path_str.contains("_home_user_project"));
        assert!(path_str.contains("abc123"));
        assert!(path_str.ends_with(".jsonl"));
    }

    #[test]
    fn test_list_sessions_empty() {
        let temp = TempDir::new().expect("test: failed to create temp dir");
        let sessions = list_sessions(temp.path(), "/test/cwd");
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_list_sessions_sorted() {
        let temp = TempDir::new().expect("test: failed to create temp dir");
        let cwd_dir = temp.path().join("_test_cwd");
        std::fs::create_dir_all(&cwd_dir).expect("test: failed to create cwd dir");

        // Create files with different timestamps (names sorted alphabetically)
        std::fs::write(cwd_dir.join("20240101_120000_sess1.jsonl"), "{}").expect("test: failed to write session file");
        std::fs::write(cwd_dir.join("20240102_120000_sess2.jsonl"), "{}").expect("test: failed to write session file");
        std::fs::write(cwd_dir.join("20240103_120000_sess3.jsonl"), "{}").expect("test: failed to write session file");

        let sessions = list_sessions(temp.path(), "/test/cwd");
        assert_eq!(sessions.len(), 3);

        // Should be reverse sorted (most recent first)
        assert!(sessions[0].to_string_lossy().contains("20240103"));
        assert!(sessions[2].to_string_lossy().contains("20240101"));
    }

    #[test]
    fn test_list_sessions_filters_non_jsonl() {
        let temp = TempDir::new().expect("test: failed to create temp dir");
        let cwd_dir = temp.path().join("_test_cwd");
        std::fs::create_dir_all(&cwd_dir).expect("test: failed to create cwd dir");

        std::fs::write(cwd_dir.join("session1.jsonl"), "{}").expect("test: failed to write jsonl file");
        std::fs::write(cwd_dir.join("session2.txt"), "{}").expect("test: failed to write txt file");
        std::fs::write(cwd_dir.join("session3.json"), "{}").expect("test: failed to write json file");

        let sessions = list_sessions(temp.path(), "/test/cwd");
        assert_eq!(sessions.len(), 1); // Only .jsonl file
    }

    #[test]
    fn test_read_entries_skips_empty_lines() {
        let temp = TempDir::new().expect("test: failed to create temp dir");
        let file = temp.path().join("test.jsonl");

        let header = make_header("test");
        let json = serde_json::to_string(&header).expect("test: failed to serialize header");

        // Write with empty lines
        std::fs::write(&file, format!("{}\n\n\n{}\n\n", json, json)).expect("test: failed to write file");

        let entries = read_entries(&file).expect("test: failed to read entries");
        assert_eq!(entries.len(), 2); // Empty lines ignored
    }

    #[test]
    fn test_purge_sessions() {
        let temp = TempDir::new().expect("test: failed to create temp dir");
        let cwd = "/test/cwd";
        let cwd_dir = temp.path().join("_test_cwd");
        std::fs::create_dir_all(&cwd_dir).expect("test: failed to create cwd dir");

        std::fs::write(cwd_dir.join("sess1.jsonl"), "{}").expect("test: failed to write session file");
        std::fs::write(cwd_dir.join("sess2.jsonl"), "{}").expect("test: failed to write session file");

        let count = purge_sessions(temp.path(), cwd).expect("test: failed to purge sessions");
        assert_eq!(count, 2);
        assert!(list_sessions(temp.path(), cwd).is_empty());
    }

    #[test]
    fn test_purge_all_sessions() {
        let temp = TempDir::new().expect("test: failed to create temp dir");

        // Create sessions in two different cwds
        let dir1 = temp.path().join("_cwd1");
        let dir2 = temp.path().join("_cwd2");
        std::fs::create_dir_all(&dir1).expect("test: failed to create dir1");
        std::fs::create_dir_all(&dir2).expect("test: failed to create dir2");

        std::fs::write(dir1.join("s1.jsonl"), "{}").expect("test: failed to write s1");
        std::fs::write(dir2.join("s2.jsonl"), "{}").expect("test: failed to write s2");
        std::fs::write(dir2.join("s3.jsonl"), "{}").expect("test: failed to write s3");

        let count = purge_all_sessions(temp.path()).expect("test: failed to purge all sessions");
        assert_eq!(count, 3);
        assert!(list_all_sessions(temp.path()).is_empty());
    }

    #[test]
    fn test_list_all_sessions() {
        let temp = TempDir::new().expect("test: failed to create temp dir");

        let dir1 = temp.path().join("_cwd1");
        let dir2 = temp.path().join("_cwd2");
        std::fs::create_dir_all(&dir1).expect("test: failed to create dir1");
        std::fs::create_dir_all(&dir2).expect("test: failed to create dir2");

        std::fs::write(dir1.join("s1.jsonl"), "{}").expect("test: failed to write s1");
        std::fs::write(dir2.join("s2.jsonl"), "{}").expect("test: failed to write s2");

        let all = list_all_sessions(temp.path());
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_read_session_summary() {
        let temp = TempDir::new().expect("test: failed to create temp dir");
        let file = temp.path().join("test.jsonl");

        let header = make_header("summ123");
        append_entry(&file, &header).expect("test: failed to append header");

        let msg = make_message(MessageId::new("m1"), None);
        append_entry(&file, &msg).expect("test: failed to append message");

        let summary = read_session_summary(&file).expect("test: failed to read summary");
        assert_eq!(summary.session_id, "summ123");
        assert_eq!(summary.model, "test-model");
        assert_eq!(summary.message_count, 1);
        assert!(summary.first_user_message.is_some());
    }

    #[test]
    fn test_read_session_summary_no_messages() {
        let temp = TempDir::new().expect("test: failed to create temp dir");
        let file = temp.path().join("test.jsonl");

        let header = make_header("empty-sess");
        append_entry(&file, &header).expect("test: failed to append header");

        let summary = read_session_summary(&file).expect("test: failed to read summary");
        assert_eq!(summary.message_count, 0);
        assert!(summary.first_user_message.is_none());
    }

    #[test]
    fn test_find_session_by_id() {
        let temp = TempDir::new().expect("test: failed to create temp dir");
        let cwd = "/test/cwd";
        let cwd_dir = temp.path().join("_test_cwd");
        std::fs::create_dir_all(&cwd_dir).expect("test: failed to create cwd dir");

        std::fs::write(cwd_dir.join("20240101_abc123.jsonl"), "{}").expect("test: failed to write session file");
        std::fs::write(cwd_dir.join("20240102_def456.jsonl"), "{}").expect("test: failed to write session file");

        let found = find_session_by_id(temp.path(), cwd, "abc123");
        assert!(found.is_some());
        assert!(found.expect("test: session should be found").to_string_lossy().contains("abc123"));

        let not_found = find_session_by_id(temp.path(), cwd, "zzz999");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_import_session() {
        let temp = TempDir::new().expect("test: failed to create temp dir");
        let source = temp.path().join("source.jsonl");
        let sessions_dir = temp.path().join("sessions");

        let header = make_header("imp1");
        append_entry(&source, &header).expect("test: failed to append header");
        let msg = make_message(MessageId::new("m1"), None);
        append_entry(&source, &msg).expect("test: failed to append message");

        let dest = import_session(&sessions_dir, &source).expect("test: failed to import session");
        assert!(dest.exists());

        // Importing again should fail (already exists)
        let result = import_session(&sessions_dir, &source);
        assert!(result.is_err());
    }
}
