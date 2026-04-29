//! Skill listing and viewing tools.
//!
//! These are read-only companions to `skill_manage`, matching Hermes-style
//! `skills_list` and `skill_view` ergonomics.

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;
use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

const SKILL_FILE_NAME: &str = "SKILL.md";
const LINKED_DIRS: [&str; 4] = ["references", "templates", "scripts", "assets"];

#[derive(Debug, Clone)]
struct SkillRecord {
    name: String,
    description: String,
    path: PathBuf,
    skill_dir: PathBuf,
    source: &'static str,
    category: Option<String>,
}

pub struct SkillsListTool {
    definition: ToolDefinition,
    global_skills_dir: PathBuf,
    project_skills_dir: Option<PathBuf>,
}

impl SkillsListTool {
    pub fn new(global_skills_dir: PathBuf, project_skills_dir: Option<PathBuf>) -> Self {
        Self {
            definition: ToolDefinition {
                name: "skills_list".to_string(),
                description: "List available reusable skills. Shows project and global skills, optional category filter, and frontmatter descriptions.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "category": {
                            "type": "string",
                            "description": "Optional category filter, e.g. 'devops'"
                        }
                    },
                    "required": []
                }),
            },
            global_skills_dir,
            project_skills_dir,
        }
    }
}

#[async_trait]
impl Tool for SkillsListTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> ToolResult {
        let category = params.get("category").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
        let mut records = discover_skill_records(&self.global_skills_dir, self.project_skills_dir.as_deref());
        if let Some(category) = category {
            records.retain(|record| record.category.as_deref() == Some(category));
        }
        if records.is_empty() {
            return ToolResult::text("No skills found.".to_string());
        }

        let mut out = format!("{} skill(s):\n", records.len());
        for record in records {
            let qualified = if let Some(category) = &record.category {
                format!("{category}/{}", record.name)
            } else {
                record.name.clone()
            };
            writeln!(
                out,
                "- **{}** [{}{}]: {}",
                qualified,
                record.source,
                record.category.as_ref().map(|category| format!("/{category}")).unwrap_or_default(),
                record.description
            )
            .ok();
            writeln!(out, "  path: {}", record.path.display()).ok();
        }
        ToolResult::text(out)
    }
}

pub struct SkillViewTool {
    definition: ToolDefinition,
    global_skills_dir: PathBuf,
    project_skills_dir: Option<PathBuf>,
}

impl SkillViewTool {
    pub fn new(global_skills_dir: PathBuf, project_skills_dir: Option<PathBuf>) -> Self {
        Self {
            definition: ToolDefinition {
                name: "skill_view".to_string(),
                description: concat!(
                    "View a skill's full SKILL.md and linked supporting files. ",
                    "Use file_path to read a file under references/, templates/, scripts/, or assets/."
                )
                .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Skill name, optionally category/name"
                        },
                        "file_path": {
                            "type": "string",
                            "description": "Optional linked file path under references/, templates/, scripts/, or assets/"
                        }
                    },
                    "required": ["name"]
                }),
            },
            global_skills_dir,
            project_skills_dir,
        }
    }
}

#[async_trait]
impl Tool for SkillViewTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> ToolResult {
        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(name) if !name.is_empty() => name,
            _ => return ToolResult::error("Missing required parameter: name"),
        };
        let records = discover_skill_records(&self.global_skills_dir, self.project_skills_dir.as_deref());
        let Some(record) = find_skill(&records, name) else {
            return ToolResult::error(format!("Skill not found: {name}"));
        };

        if let Some(file_path) = params.get("file_path").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
            return view_linked_file(&record, file_path);
        }

        match std::fs::read_to_string(&record.path) {
            Ok(content) => {
                let linked = linked_files(&record.skill_dir);
                let mut out = format!(
                    "Skill: {}\nSource: {}\nPath: {}\n\n{}",
                    display_name(&record),
                    record.source,
                    record.path.display(),
                    content
                );
                if !linked.is_empty() {
                    writeln!(out, "\n\nLinked files:").ok();
                    for (dir, files) in linked {
                        writeln!(out, "  {dir}:").ok();
                        for file in files {
                            writeln!(out, "    - {file}").ok();
                        }
                    }
                }
                ToolResult::text(out)
            }
            Err(err) => ToolResult::error(format!("Failed to read {}: {err}", record.path.display())),
        }
    }
}

fn discover_skill_records(global_dir: &Path, project_dir: Option<&Path>) -> Vec<SkillRecord> {
    let mut records = Vec::new();
    records.extend(scan_skill_root(global_dir, "global"));
    if let Some(project_dir) = project_dir {
        records.extend(scan_skill_root(project_dir, "project"));
    }

    // Project skills should override global entries with the same display name.
    let mut seen = HashSet::new();
    records.sort_by(|a, b| {
        source_rank(b.source)
            .cmp(&source_rank(a.source))
            .then_with(|| display_name(a).cmp(&display_name(b)))
    });
    records.retain(|record| seen.insert(display_name(record)));
    records.sort_by(|a, b| display_name(a).cmp(&display_name(b)));
    records
}

fn source_rank(source: &str) -> u8 {
    match source {
        "project" => 2,
        "global" => 1,
        _ => 0,
    }
}

fn scan_skill_root(root: &Path, source: &'static str) -> Vec<SkillRecord> {
    let mut records = Vec::new();
    if !root.is_dir() {
        return records;
    }
    scan_skill_root_inner(root, root, source, &mut records);
    records
}

fn scan_skill_root_inner(root: &Path, current: &Path, source: &'static str, records: &mut Vec<SkillRecord>) {
    let Ok(entries) = std::fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_file = path.join(SKILL_FILE_NAME);
        if skill_file.is_file() {
            if let Some(record) = load_record(root, &skill_file, source) {
                records.push(record);
            }
            continue;
        }
        scan_skill_root_inner(root, &path, source, records);
    }
}

fn load_record(root: &Path, skill_file: &Path, source: &'static str) -> Option<SkillRecord> {
    let content = std::fs::read_to_string(skill_file).ok()?;
    let skill_dir = skill_file.parent()?.to_path_buf();
    let name = extract_frontmatter_field(&content, "name")
        .or_else(|| skill_dir.file_name().map(|name| name.to_string_lossy().to_string()))?;
    let description = extract_frontmatter_field(&content, "description").unwrap_or_else(|| first_body_line(&content));
    let rel_dir = skill_dir.strip_prefix(root).ok()?;
    let category = rel_dir.parent().and_then(|parent| {
        if parent.as_os_str().is_empty() {
            None
        } else {
            Some(parent.to_string_lossy().to_string())
        }
    });
    Some(SkillRecord {
        name,
        description,
        path: skill_file.to_path_buf(),
        skill_dir,
        source,
        category,
    })
}

fn extract_frontmatter_field(content: &str, key: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        let Some((found_key, value)) = line.split_once(':') else {
            continue;
        };
        if found_key.trim() == key {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn first_body_line(content: &str) -> String {
    let mut in_frontmatter = false;
    for (idx, line) in content.lines().enumerate() {
        if idx == 0 && line.trim() == "---" {
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if line.trim() == "---" {
                in_frontmatter = false;
            }
            continue;
        }
        if !line.trim().is_empty() {
            return line.trim().trim_start_matches('#').trim().to_string();
        }
    }
    "(no description)".to_string()
}

fn display_name(record: &SkillRecord) -> String {
    if let Some(category) = &record.category {
        format!("{category}/{}", record.name)
    } else {
        record.name.clone()
    }
}

fn find_skill(records: &[SkillRecord], name: &str) -> Option<SkillRecord> {
    records.iter().find(|record| display_name(record) == name || record.name == name).cloned()
}

fn linked_files(skill_dir: &Path) -> BTreeMap<String, Vec<String>> {
    let mut grouped = BTreeMap::new();
    for dir in LINKED_DIRS {
        let root = skill_dir.join(dir);
        let mut files = Vec::new();
        collect_files(&root, &root, &mut files);
        if !files.is_empty() {
            files.sort();
            grouped.insert(dir.to_string(), files.into_iter().map(|file| format!("{dir}/{file}")).collect());
        }
    }
    grouped
}

fn collect_files(root: &Path, current: &Path, files: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files);
        } else if path.is_file()
            && let Ok(rel) = path.strip_prefix(root)
        {
            files.push(rel.to_string_lossy().to_string());
        }
    }
}

fn view_linked_file(record: &SkillRecord, file_path: &str) -> ToolResult {
    let relative = Path::new(file_path);
    if let Err(err) = clankers_skills::validate_supporting_path(relative) {
        return ToolResult::error(err.to_string());
    }
    let target = record.skill_dir.join(relative);
    if !target.is_file() {
        return ToolResult::error(format!("Linked file not found: {file_path}"));
    }
    match std::fs::read_to_string(&target) {
        Ok(content) => ToolResult::text(format!("{}:{}\n\n{}", display_name(record), file_path, content)),
        Err(err) => ToolResult::error(format!("Failed to read {}: {err}", target.display())),
    }
}

pub fn project_skills_dir_from_cwd() -> Option<PathBuf> {
    std::env::current_dir().ok().map(|cwd| crate::config::ProjectPaths::resolve(&cwd).skills_dir)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;

    use super::*;

    const SKILL: &str = "---\nname: test-skill\ndescription: A useful test skill\n---\n# Body\nUse it.\n";

    fn ctx() -> ToolContext {
        ToolContext::new("skill-view-test".to_string(), CancellationToken::new(), None)
    }

    fn text(result: &ToolResult) -> String {
        result
            .content
            .iter()
            .filter_map(|content| match content {
                crate::tools::ToolResultContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn write_skill(root: &Path, rel: &str) -> PathBuf {
        let dir = root.join(rel);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(SKILL_FILE_NAME), SKILL).unwrap();
        dir
    }

    #[tokio::test]
    async fn skills_list_lists_skill_with_description() {
        let global = TempDir::new().unwrap();
        write_skill(global.path(), "test-skill");
        let tool = SkillsListTool::new(global.path().to_path_buf(), None);
        let result = tool.execute(&ctx(), json!({})).await;
        let out = text(&result);
        assert!(out.contains("test-skill"));
        assert!(out.contains("A useful test skill"));
    }

    #[tokio::test]
    async fn skills_list_filters_category() {
        let global = TempDir::new().unwrap();
        write_skill(global.path(), "devops/test-skill");
        write_skill(global.path(), "docs/other-skill");
        let tool = SkillsListTool::new(global.path().to_path_buf(), None);
        let result = tool.execute(&ctx(), json!({"category": "devops"})).await;
        let out = text(&result);
        assert!(out.contains("devops/test-skill"));
        assert!(!out.contains("docs/other-skill"));
    }

    #[tokio::test]
    async fn skill_view_shows_skill_and_linked_files() {
        let global = TempDir::new().unwrap();
        let skill_dir = write_skill(global.path(), "test-skill");
        std::fs::create_dir_all(skill_dir.join("references")).unwrap();
        std::fs::write(skill_dir.join("references/api.md"), "API docs").unwrap();
        let tool = SkillViewTool::new(global.path().to_path_buf(), None);
        let result = tool.execute(&ctx(), json!({"name": "test-skill"})).await;
        let out = text(&result);
        assert!(out.contains("# Body"));
        assert!(out.contains("references/api.md"));
    }

    #[tokio::test]
    async fn skill_view_reads_linked_file() {
        let global = TempDir::new().unwrap();
        let skill_dir = write_skill(global.path(), "test-skill");
        std::fs::create_dir_all(skill_dir.join("scripts")).unwrap();
        std::fs::write(skill_dir.join("scripts/run.sh"), "echo ok").unwrap();
        let tool = SkillViewTool::new(global.path().to_path_buf(), None);
        let result = tool.execute(&ctx(), json!({"name": "test-skill", "file_path": "scripts/run.sh"})).await;
        assert!(text(&result).contains("echo ok"));
    }

    #[tokio::test]
    async fn skill_view_rejects_path_traversal() {
        let global = TempDir::new().unwrap();
        write_skill(global.path(), "test-skill");
        let tool = SkillViewTool::new(global.path().to_path_buf(), None);
        let result = tool.execute(&ctx(), json!({"name": "test-skill", "file_path": "../secret"})).await;
        assert!(result.is_error);
        assert!(text(&result).contains("supporting file path"));
    }
}
