//! Change lifecycle management

use std::path::Path;
use std::path::PathBuf;

use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeInfo {
    pub name: String,
    pub path: PathBuf,
    pub schema: String,
    pub created_at: String,
    pub task_progress: Option<(usize, usize)>, // (completed, total)
}

/// Create a new change directory with scaffolding
pub fn create_change(changes_dir: &Path, name: &str, schema: &str) -> std::io::Result<PathBuf> {
    let change_dir = changes_dir.join(name);
    std::fs::create_dir_all(&change_dir)?;
    std::fs::create_dir_all(change_dir.join("specs"))?;

    // Write .openspec.yaml metadata
    let metadata = format!("schema: {}\ncreated: {}\n", schema, Utc::now().to_rfc3339());
    std::fs::write(change_dir.join(".openspec.yaml"), metadata)?;

    Ok(change_dir)
}

/// List active changes
pub fn list_changes(changes_dir: &Path) -> Vec<ChangeInfo> {
    let mut changes = Vec::new();
    let entries = match std::fs::read_dir(changes_dir) {
        Ok(e) => e,
        Err(_) => return changes,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        if name == "archive" {
            continue;
        }

        // Read metadata
        let meta_path = path.join(".openspec.yaml");
        let (schema, created) = if let Ok(meta) = std::fs::read_to_string(&meta_path) {
            parse_change_meta(&meta)
        } else {
            ("unknown".to_string(), String::new())
        };

        let tasks = parse_task_progress(&path.join("tasks.md"));

        changes.push(ChangeInfo {
            name,
            path,
            schema,
            created_at: created,
            task_progress: tasks,
        });
    }
    changes.sort_by(|a, b| a.name.cmp(&b.name));
    changes
}

/// Archive a completed change
pub fn archive_change(changes_dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    let source = changes_dir.join(name);
    let archive_dir = changes_dir.join("archive");
    std::fs::create_dir_all(&archive_dir)?;
    let date = Utc::now().format("%Y-%m-%d");
    let dest = archive_dir.join(format!("{}-{}", date, name));
    // Use fs::rename for atomic move (same filesystem)
    std::fs::rename(&source, &dest)?;
    Ok(dest)
}

fn parse_change_meta(content: &str) -> (String, String) {
    let mut schema = String::new();
    let mut created = String::new();
    for line in content.lines() {
        if let Some(v) = line.strip_prefix("schema:") {
            schema = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("created:") {
            created = v.trim().to_string();
        }
    }
    (schema, created)
}

/// Count [x] vs [ ] in tasks.md
fn parse_task_progress(path: &Path) -> Option<(usize, usize)> {
    let content = std::fs::read_to_string(path).ok()?;
    let total = content.matches("[ ]").count() + content.matches("[x]").count() + content.matches("[X]").count();
    let done = content.matches("[x]").count() + content.matches("[X]").count();
    if total > 0 { Some((done, total)) } else { None }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_create_change() {
        let dir = TempDir::new().unwrap();
        let change_path = create_change(dir.path(), "test-change", "spec-driven").unwrap();

        assert!(change_path.exists());
        assert!(change_path.join("specs").exists());
        assert!(change_path.join(".openspec.yaml").exists());

        let meta = std::fs::read_to_string(change_path.join(".openspec.yaml")).unwrap();
        assert!(meta.contains("schema: spec-driven"));
    }

    #[test]
    fn test_list_empty_changes() {
        let dir = TempDir::new().unwrap();
        let changes = list_changes(dir.path());
        assert!(changes.is_empty());
    }

    #[test]
    fn test_list_changes() {
        let dir = TempDir::new().unwrap();
        create_change(dir.path(), "change1", "spec-driven").unwrap();
        create_change(dir.path(), "change2", "minimal").unwrap();

        let changes = list_changes(dir.path());
        assert_eq!(changes.len(), 2);
        assert!(changes.iter().any(|c| c.name == "change1"));
        assert!(changes.iter().any(|c| c.name == "change2"));
    }

    #[test]
    fn test_archive_change() {
        let dir = TempDir::new().unwrap();
        let change_path = create_change(dir.path(), "old-change", "spec-driven").unwrap();
        std::fs::write(change_path.join("data.txt"), "test").unwrap();

        let archived = archive_change(dir.path(), "old-change").unwrap();
        assert!(archived.exists());
        assert!(archived.join("data.txt").exists());
        assert!(!change_path.exists());
    }

    #[test]
    fn test_task_progress_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("tasks.md");
        std::fs::write(&path, "# Tasks\n\nNo checkboxes here").unwrap();

        let progress = parse_task_progress(&path);
        assert!(progress.is_none());
    }

    #[test]
    fn test_task_progress_partial() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("tasks.md");
        std::fs::write(&path, "- [x] Done\n- [ ] Todo\n- [X] Also done").unwrap();

        let progress = parse_task_progress(&path);
        assert_eq!(progress, Some((2, 3)));
    }
}
