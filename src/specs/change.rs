//! Change lifecycle management

use std::path::Path;
use std::path::PathBuf;

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeInfo {
    pub name: String,
    pub path: PathBuf,
    pub schema: String,
    pub created_at: String,
    pub task_progress: Option<TaskProgress>,
}

/// Task progress summary for a change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgress {
    pub done: usize,
    pub in_progress: usize,
    pub todo: usize,
    pub total: usize,
    pub tasks: Vec<TaskItem>,
}

/// A single task line parsed from tasks.md
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub description: String,
    pub state: TaskState,
}

/// Task state with optional timing metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskState {
    Todo,
    InProgress {
        started: Option<DateTime<Utc>>,
    },
    Done {
        started: Option<DateTime<Utc>>,
        completed: Option<DateTime<Utc>>,
        duration: Option<String>,
    },
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

        let task_progress = parse_task_progress(&path.join("tasks.md"));

        changes.push(ChangeInfo {
            name,
            path,
            schema,
            created_at: created,
            task_progress,
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

/// Parse tasks from tasks.md, extracting state and timing metadata.
///
/// Recognized task line formats:
///   - [ ] Description                                          → Todo
///   - [~] Description ⏱ started: 2026-03-03T11:22Z            → InProgress
///   - [x] Description ✅ 2h 15m (started: … → completed: …)   → Done
///   - [x] Description                                          → Done (no timing)
fn parse_task_progress(path: &Path) -> Option<TaskProgress> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut tasks = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim_start_matches(|c: char| c == '-' || c == '*' || c.is_whitespace());
        if let Some(rest) = trimmed.strip_prefix("[ ] ") {
            tasks.push(TaskItem {
                description: rest.to_string(),
                state: TaskState::Todo,
            });
        } else if let Some(rest) = trimmed.strip_prefix("[~] ") {
            let (desc, started) = parse_in_progress(rest);
            tasks.push(TaskItem {
                description: desc,
                state: TaskState::InProgress { started },
            });
        } else if trimmed.starts_with("[x] ") || trimmed.starts_with("[X] ") {
            let rest = &trimmed[4..];
            let (desc, started, completed, duration) = parse_done(rest);
            tasks.push(TaskItem {
                description: desc,
                state: TaskState::Done { started, completed, duration },
            });
        }
    }

    if tasks.is_empty() {
        return None;
    }

    let done = tasks.iter().filter(|t| matches!(t.state, TaskState::Done { .. })).count();
    let in_progress = tasks.iter().filter(|t| matches!(t.state, TaskState::InProgress { .. })).count();
    let todo = tasks.iter().filter(|t| matches!(t.state, TaskState::Todo)).count();
    let total = tasks.len();

    Some(TaskProgress { done, in_progress, todo, total, tasks })
}

/// Parse an in-progress line: "Description ⏱ started: 2026-03-03T11:22Z"
fn parse_in_progress(rest: &str) -> (String, Option<DateTime<Utc>>) {
    if let Some(idx) = rest.find("⏱") {
        let desc = rest[..idx].trim().to_string();
        let meta = rest[idx..].trim();
        let started = extract_timestamp(meta, "started:");
        (desc, started)
    } else {
        (rest.to_string(), None)
    }
}

/// Parse a done line: "Description ✅ 2h 15m (started: … → completed: …)"
fn parse_done(rest: &str) -> (String, Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<String>) {
    if let Some(idx) = rest.find('✅') {
        let desc = rest[..idx].trim().to_string();
        let meta = rest[idx + '✅'.len_utf8()..].trim();
        let (duration, timestamps) = if let Some(paren_start) = meta.find('(') {
            let dur = meta[..paren_start].trim().to_string();
            let ts = &meta[paren_start..];
            (if dur.is_empty() { None } else { Some(dur) }, ts)
        } else {
            let dur = meta.to_string();
            (if dur.is_empty() { None } else { Some(dur) }, "")
        };
        let started = extract_timestamp(timestamps, "started:");
        let completed = extract_timestamp(timestamps, "completed:");
        (desc, started, completed, duration)
    } else {
        (rest.to_string(), None, None, None)
    }
}

/// Extract a timestamp value after a label like "started:" or "completed:"
fn extract_timestamp(text: &str, label: &str) -> Option<DateTime<Utc>> {
    let idx = text.find(label)?;
    let after = text[idx + label.len()..].trim();
    // Take characters until we hit a delimiter: →, ), or end of string
    let ts_str: String = after.chars().take_while(|c| !matches!(c, '→' | ')' | '\n')).collect();
    let ts_str = ts_str.trim();
    // Try parsing with various formats
    parse_flexible_timestamp(ts_str)
}

/// Parse a timestamp flexibly: full RFC 3339, or minute-precision (YYYY-MM-DDTHH:MMZ)
fn parse_flexible_timestamp(s: &str) -> Option<DateTime<Utc>> {
    // Full RFC 3339 / ISO 8601 (e.g. 2026-03-03T11:22:00Z, 2026-03-03T11:22:00+00:00)
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    // Minute-precision with Z suffix: 2026-03-03T11:22Z
    // chrono can't parse bare "Z" as a timezone, so strip it and add ":00Z" to make RFC 3339
    if let Some(prefix) = s.strip_suffix('Z') {
        let with_seconds = format!("{}:00Z", prefix);
        if let Ok(dt) = DateTime::parse_from_rfc3339(&with_seconds) {
            return Some(dt.with_timezone(&Utc));
        }
    }
    // Minute-precision with offset: 2026-03-03T11:22+00:00
    if let Ok(dt) = DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M%:z") {
        return Some(dt.with_timezone(&Utc));
    }
    None
}

/// Format a duration between two timestamps as a human-friendly string.
/// E.g., "15m", "1h 30m", "2d 4h".
pub fn format_duration_human(start: &DateTime<Utc>, end: &DateTime<Utc>) -> String {
    let dur = *end - *start;
    let total_minutes = dur.num_minutes().unsigned_abs();
    let days = total_minutes / (60 * 24);
    let hours = (total_minutes % (60 * 24)) / 60;
    let minutes = total_minutes % 60;

    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{}d", days));
    }
    if hours > 0 {
        parts.push(format!("{}h", hours));
    }
    if minutes > 0 || parts.is_empty() {
        parts.push(format!("{}m", minutes));
    }
    parts.join(" ")
}

/// Format a task line for writing back to tasks.md.
pub fn format_task_line(task: &TaskItem) -> String {
    match &task.state {
        TaskState::Todo => format!("- [ ] {}", task.description),
        TaskState::InProgress { started } => {
            if let Some(ts) = started {
                format!("- [~] {} ⏱ started: {}", task.description, ts.format("%Y-%m-%dT%H:%MZ"))
            } else {
                format!("- [~] {}", task.description)
            }
        }
        TaskState::Done { started, completed, duration } => {
            let mut line = format!("- [x] {}", task.description);
            let has_timing = duration.is_some() || started.is_some() || completed.is_some();
            if has_timing {
                line.push_str(" ✅");
                if let Some(dur) = duration {
                    line.push_str(&format!(" {}", dur));
                }
                if started.is_some() || completed.is_some() {
                    line.push_str(" (");
                    if let Some(ts) = started {
                        line.push_str(&format!("started: {}", ts.format("%Y-%m-%dT%H:%MZ")));
                    }
                    if let Some(ts) = completed {
                        if started.is_some() {
                            line.push_str(" → ");
                        }
                        line.push_str(&format!("completed: {}", ts.format("%Y-%m-%dT%H:%MZ")));
                    }
                    line.push(')');
                }
            }
            line
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_create_change() {
        let dir = TempDir::new().expect("failed to create temp dir for test");
        let change_path = create_change(dir.path(), "test-change", "spec-driven").expect("failed to create change");

        assert!(change_path.exists());
        assert!(change_path.join("specs").exists());
        assert!(change_path.join(".openspec.yaml").exists());

        let meta = std::fs::read_to_string(change_path.join(".openspec.yaml")).expect("failed to read metadata file");
        assert!(meta.contains("schema: spec-driven"));
    }

    #[test]
    fn test_list_empty_changes() {
        let dir = TempDir::new().expect("failed to create temp dir for test");
        let changes = list_changes(dir.path());
        assert!(changes.is_empty());
    }

    #[test]
    fn test_list_changes() {
        let dir = TempDir::new().expect("failed to create temp dir for test");
        create_change(dir.path(), "change1", "spec-driven").expect("failed to create change1");
        create_change(dir.path(), "change2", "minimal").expect("failed to create change2");

        let changes = list_changes(dir.path());
        assert_eq!(changes.len(), 2);
        assert!(changes.iter().any(|c| c.name == "change1"));
        assert!(changes.iter().any(|c| c.name == "change2"));
    }

    #[test]
    fn test_archive_change() {
        let dir = TempDir::new().expect("failed to create temp dir for test");
        let change_path = create_change(dir.path(), "old-change", "spec-driven").expect("failed to create change");
        std::fs::write(change_path.join("data.txt"), "test").expect("failed to write test data");

        let archived = archive_change(dir.path(), "old-change").expect("failed to archive change");
        assert!(archived.exists());
        assert!(archived.join("data.txt").exists());
        assert!(!change_path.exists());
    }

    #[test]
    fn test_task_progress_none() {
        let dir = TempDir::new().expect("failed to create temp dir for test");
        let path = dir.path().join("tasks.md");
        std::fs::write(&path, "# Tasks\n\nNo checkboxes here").expect("failed to write tasks file");

        let progress = parse_task_progress(&path);
        assert!(progress.is_none());
    }

    #[test]
    fn test_task_progress_partial() {
        let dir = TempDir::new().expect("failed to create temp dir for test");
        let path = dir.path().join("tasks.md");
        std::fs::write(&path, "- [x] Done\n- [ ] Todo\n- [X] Also done").expect("failed to write tasks file");

        let progress = parse_task_progress(&path).expect("failed to parse task progress");
        assert_eq!(progress.done, 2);
        assert_eq!(progress.todo, 1);
        assert_eq!(progress.in_progress, 0);
        assert_eq!(progress.total, 3);
    }

    #[test]
    fn test_task_progress_with_in_progress() {
        let dir = TempDir::new().expect("failed to create temp dir for test");
        let path = dir.path().join("tasks.md");
        std::fs::write(
            &path,
            "- [x] Done task\n- [~] Working on it ⏱ started: 2026-03-03T11:22Z\n- [ ] Not started",
        )
        .expect("failed to write tasks file");

        let progress = parse_task_progress(&path).expect("failed to parse task progress");
        assert_eq!(progress.done, 1);
        assert_eq!(progress.in_progress, 1);
        assert_eq!(progress.todo, 1);
        assert_eq!(progress.total, 3);

        // Verify the in-progress task has a parsed timestamp
        let wip = &progress.tasks[1];
        assert!(matches!(&wip.state, TaskState::InProgress { started: Some(_) }));
    }

    #[test]
    fn test_task_progress_done_with_timing() {
        let dir = TempDir::new().expect("failed to create temp dir for test");
        let path = dir.path().join("tasks.md");
        std::fs::write(
            &path,
            "- [x] Feature A ✅ 2h 15m (started: 2026-03-03T09:00Z → completed: 2026-03-03T11:15Z)",
        )
        .expect("failed to write tasks file");

        let progress = parse_task_progress(&path).expect("failed to parse task progress");
        assert_eq!(progress.done, 1);
        assert_eq!(progress.total, 1);

        let task = &progress.tasks[0];
        assert_eq!(task.description, "Feature A");
        match &task.state {
            TaskState::Done { started, completed, duration } => {
                assert!(started.is_some());
                assert!(completed.is_some());
                assert_eq!(duration.as_deref(), Some("2h 15m"));
            }
            _ => panic!("expected Done state"),
        }
    }

    #[test]
    fn test_task_in_progress_no_timestamp() {
        let dir = TempDir::new().expect("failed to create temp dir for test");
        let path = dir.path().join("tasks.md");
        std::fs::write(&path, "- [~] Working on it").expect("failed to write tasks file");

        let progress = parse_task_progress(&path).expect("failed to parse task progress");
        assert_eq!(progress.in_progress, 1);
        let task = &progress.tasks[0];
        assert!(matches!(&task.state, TaskState::InProgress { started: None }));
    }

    #[test]
    fn test_format_task_line_todo() {
        let task = TaskItem { description: "Do something".to_string(), state: TaskState::Todo };
        assert_eq!(format_task_line(&task), "- [ ] Do something");
    }

    #[test]
    fn test_format_task_line_in_progress_with_timestamp() {
        let started = DateTime::parse_from_rfc3339("2026-03-03T11:22:00Z").expect("failed to parse RFC3339 timestamp").with_timezone(&Utc);
        let task = TaskItem {
            description: "Working".to_string(),
            state: TaskState::InProgress { started: Some(started) },
        };
        assert_eq!(format_task_line(&task), "- [~] Working ⏱ started: 2026-03-03T11:22Z");
    }

    #[test]
    fn test_format_task_line_done_with_full_timing() {
        let started = DateTime::parse_from_rfc3339("2026-03-03T09:00:00Z").expect("failed to parse RFC3339 timestamp").with_timezone(&Utc);
        let completed = DateTime::parse_from_rfc3339("2026-03-03T11:15:00Z").expect("failed to parse RFC3339 timestamp").with_timezone(&Utc);
        let task = TaskItem {
            description: "Feature".to_string(),
            state: TaskState::Done {
                started: Some(started),
                completed: Some(completed),
                duration: Some("2h 15m".to_string()),
            },
        };
        assert_eq!(
            format_task_line(&task),
            "- [x] Feature ✅ 2h 15m (started: 2026-03-03T09:00Z → completed: 2026-03-03T11:15Z)"
        );
    }

    #[test]
    fn test_format_duration_human() {
        let start = DateTime::parse_from_rfc3339("2026-03-03T09:00:00Z").expect("failed to parse RFC3339 timestamp").with_timezone(&Utc);
        let end_15m = DateTime::parse_from_rfc3339("2026-03-03T09:15:00Z").expect("failed to parse RFC3339 timestamp").with_timezone(&Utc);
        assert_eq!(format_duration_human(&start, &end_15m), "15m");

        let end_1h30 = DateTime::parse_from_rfc3339("2026-03-03T10:30:00Z").expect("failed to parse RFC3339 timestamp").with_timezone(&Utc);
        assert_eq!(format_duration_human(&start, &end_1h30), "1h 30m");

        let end_2d4h = DateTime::parse_from_rfc3339("2026-03-05T13:00:00Z").expect("failed to parse RFC3339 timestamp").with_timezone(&Utc);
        assert_eq!(format_duration_human(&start, &end_2d4h), "2d 4h");
    }

    #[test]
    fn test_roundtrip_parse_format() {
        let dir = TempDir::new().expect("failed to create temp dir for test");
        let path = dir.path().join("tasks.md");
        let content = "\
- [ ] Not started\n\
- [~] In progress ⏱ started: 2026-03-03T11:22Z\n\
- [x] Done plain\n\
- [x] Done timed ✅ 1h 30m (started: 2026-03-03T09:00Z → completed: 2026-03-03T10:30Z)\n";
        std::fs::write(&path, content).expect("failed to write tasks file");

        let progress = parse_task_progress(&path).expect("failed to parse task progress");
        assert_eq!(progress.total, 4);

        // Format back and re-parse — should be stable
        let formatted: Vec<String> = progress.tasks.iter().map(format_task_line).collect();
        let rejoined = formatted.join("\n") + "\n";
        std::fs::write(&path, &rejoined).expect("failed to write reformatted tasks file");

        let progress2 = parse_task_progress(&path).expect("failed to parse task progress after roundtrip");
        assert_eq!(progress2.total, 4);
        assert_eq!(progress2.done, 2);
        assert_eq!(progress2.in_progress, 1);
        assert_eq!(progress2.todo, 1);
    }
}
