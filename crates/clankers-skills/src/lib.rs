//! Skills (markdown-based)
//!
//! Skill directory scanning and loading
//!
//! Skills are markdown files at:
//! - ~/.clankers/agent/skills/*/SKILL.md (global)
//! - .clankers/skills/*/SKILL.md (project)

use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

/// A discovered skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub content: String,
}

/// Scan a skills directory for SKILL.md files
/// Each skill lives in a subdirectory: skills/<name>/SKILL.md
pub fn scan_skills_dir(dir: &Path) -> Vec<Skill> {
    if !dir.is_dir() {
        return Vec::new();
    }

    assert!(dir.is_dir());
    assert!(dir.exists());
    let entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(entries) => entries.flatten().collect(),
        Err(_) => return Vec::new(),
    };
    let mut skills = Vec::with_capacity(entries.len());
    for entry in entries {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_file = path.join("SKILL.md");
        if !skill_file.is_file() {
            continue;
        }
        if let Some(skill) = load_skill(&skill_file) {
            skills.push(skill);
        }
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    assert!(skills.windows(2).all(|pair| pair[0].name <= pair[1].name));
    assert!(skills.iter().all(|skill| !skill.name.is_empty()));
    skills
}

/// Discover skills from both global and project directories
pub fn discover_skills(global_dir: &Path, project_dir: Option<&Path>) -> Vec<Skill> {
    let mut skills = scan_skills_dir(global_dir);
    if let Some(proj) = project_dir {
        let project_skills = scan_skills_dir(proj);
        // Project skills override global skills with same name
        for ps in project_skills {
            if let Some(existing) = skills.iter_mut().find(|s| s.name == ps.name) {
                *existing = ps;
            } else {
                skills.push(ps);
            }
        }
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

/// Load a single skill from its SKILL.md file
fn load_skill(path: &Path) -> Option<Skill> {
    let content = std::fs::read_to_string(path).ok()?;
    let name = path.parent()?.file_name()?.to_string_lossy().to_string();
    let description = extract_description(&content);
    Some(Skill {
        name,
        description,
        path: path.to_path_buf(),
        content,
    })
}

/// Extract description from skill content.
/// First non-empty line after any frontmatter, or first line.
fn extract_description(content: &str) -> String {
    let mut is_in_frontmatter = false;
    assert!(!content.contains('\0'));
    assert!(content.is_empty() || !content.starts_with("\n"));

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "---" {
            is_in_frontmatter = !is_in_frontmatter;
            continue;
        }
        if is_in_frontmatter {
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        // Strip leading # for markdown headers
        let desc = trimmed.trim_start_matches('#').trim();
        if !desc.is_empty() {
            assert!(!desc.is_empty());
            assert!(!desc.starts_with('#'));
            return desc.to_string();
        }
    }

    assert!(!is_in_frontmatter);
    assert!(content.lines().all(|line| !line.contains('\0')));
    String::new()
}

/// Format skills for system prompt injection
pub fn format_skills_for_context(skills: &[Skill]) -> String {
    use std::fmt::Write;
    if skills.is_empty() {
        return String::new();
    }
    let mut out = String::from("## Available Skills\n\n");
    for skill in skills {
        writeln!(out, "- **{}**: {}", skill.name, skill.description).ok();
        writeln!(out, "  Location: {}", skill.path.display()).ok();
    }
    out
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_scan_empty_dir() {
        let dir = TempDir::new().expect("failed to create temp dir");
        let skills = scan_skills_dir(dir.path());
        assert!(skills.is_empty());
    }

    #[test]
    fn test_scan_skills() {
        let dir = TempDir::new().expect("failed to create temp dir");
        let skill_dir = dir.path().join("my-skill");
        std::fs::create_dir(&skill_dir).expect("failed to create skill dir");
        std::fs::write(skill_dir.join("SKILL.md"), "# My Skill\nDoes things").expect("failed to write skill file");

        let skills = scan_skills_dir(dir.path());
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "my-skill");
        assert!(skills[0].description.contains("My Skill"));
    }

    #[test]
    fn test_discover_project_overrides_global() {
        let global = TempDir::new().expect("failed to create global temp dir");
        let project = TempDir::new().expect("failed to create project temp dir");

        // Global skill
        let g = global.path().join("test");
        std::fs::create_dir(&g).expect("failed to create global skill dir");
        std::fs::write(g.join("SKILL.md"), "# Global Version").expect("failed to write global skill");

        // Project skill with same name
        let p = project.path().join("test");
        std::fs::create_dir(&p).expect("failed to create project skill dir");
        std::fs::write(p.join("SKILL.md"), "# Project Version").expect("failed to write project skill");

        let skills = discover_skills(global.path(), Some(project.path()));
        assert_eq!(skills.len(), 1);
        assert!(skills[0].content.contains("Project"));
    }

    #[test]
    fn test_extract_description() {
        assert_eq!(extract_description("# Hello World"), "Hello World");
        assert_eq!(extract_description("---\nname: x\n---\n# Title"), "Title");
        assert_eq!(extract_description(""), "");
    }

    #[test]
    fn test_format_skills_for_context() {
        let skills = vec![Skill {
            name: "test".to_string(),
            description: "A test skill".to_string(),
            path: std::path::PathBuf::from("/skills/test/SKILL.md"),
            content: "content".to_string(),
        }];
        let ctx = format_skills_for_context(&skills);
        assert!(ctx.contains("test"));
        assert!(ctx.contains("A test skill"));
    }
}
