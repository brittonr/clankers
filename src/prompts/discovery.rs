//! Prompt template scanning and loading
//!
//! Prompts are markdown files at:
//! - ~/.clankers/agent/prompts/*.md (global)
//! - .clankers/prompts/*.md (project)

use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

/// A discovered prompt template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub content: String,
    pub variables: Vec<String>,
}

/// Scan a prompts directory for *.md files
pub fn scan_prompts_dir(dir: &Path) -> Vec<PromptTemplate> {
    let mut prompts = Vec::new();
    if !dir.is_dir() {
        return prompts;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return prompts,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if let Some(prompt) = load_prompt(&path) {
            prompts.push(prompt);
        }
    }
    prompts.sort_by(|a, b| a.name.cmp(&b.name));
    prompts
}

/// Discover prompts from both global and project directories
pub fn discover_prompts(global_dir: &Path, project_dir: Option<&Path>) -> Vec<PromptTemplate> {
    let mut prompts = scan_prompts_dir(global_dir);
    if let Some(proj) = project_dir {
        let project_prompts = scan_prompts_dir(proj);
        for pp in project_prompts {
            if let Some(existing) = prompts.iter_mut().find(|p| p.name == pp.name) {
                *existing = pp;
            } else {
                prompts.push(pp);
            }
        }
    }
    prompts.sort_by(|a, b| a.name.cmp(&b.name));
    prompts
}

/// Load a single prompt template from a .md file
fn load_prompt(path: &Path) -> Option<PromptTemplate> {
    let content = std::fs::read_to_string(path).ok()?;
    let name = path.file_stem()?.to_string_lossy().to_string();
    let description = extract_description(&content);
    let variables = extract_variables(&content);
    Some(PromptTemplate {
        name,
        description,
        path: path.to_path_buf(),
        content,
        variables,
    })
}

/// Extract description: first non-empty line after frontmatter
fn extract_description(content: &str) -> String {
    let mut in_frontmatter = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "---" {
            in_frontmatter = !in_frontmatter;
            continue;
        }
        if in_frontmatter || trimmed.is_empty() {
            continue;
        }
        return trimmed.trim_start_matches('#').trim().to_string();
    }
    String::new()
}

/// Extract {{variable}} names from template content
fn extract_variables(content: &str) -> Vec<String> {
    let mut vars = Vec::new();
    let mut remaining = content;
    while let Some(start) = remaining.find("{{") {
        remaining = &remaining[start + 2..];
        if let Some(end) = remaining.find("}}") {
            let var = remaining[..end].trim().to_string();
            if !var.is_empty() && !vars.contains(&var) {
                vars.push(var);
            }
            remaining = &remaining[end + 2..];
        }
    }
    vars
}

/// Expand a prompt template with variable values
pub fn expand_template(template: &str, vars: &std::collections::HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("{{{{{}}}}}", key), value);
    }
    result
}

/// Format prompts for display (command palette)
pub fn format_prompts_list(prompts: &[PromptTemplate]) -> String {
    if prompts.is_empty() {
        return String::from("No prompt templates found.");
    }
    let mut out = String::new();
    for p in prompts {
        out.push_str(&format!("/{} — {}\n", p.name, p.description));
        if !p.variables.is_empty() {
            out.push_str(&format!("  Variables: {}\n", p.variables.join(", ")));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_scan_empty_dir() {
        let dir = TempDir::new().unwrap();
        let prompts = scan_prompts_dir(dir.path());
        assert!(prompts.is_empty());
    }

    #[test]
    fn test_scan_prompts() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("review.md"), "# Code Review\nReview the code").unwrap();

        let prompts = scan_prompts_dir(dir.path());
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].name, "review");
    }

    #[test]
    fn test_extract_variables() {
        let vars = extract_variables("Hello {{name}}, welcome to {{project}}");
        assert_eq!(vars, vec!["name", "project"]);
    }

    #[test]
    fn test_extract_variables_no_duplicates() {
        let vars = extract_variables("{{x}} and {{x}} again");
        assert_eq!(vars, vec!["x"]);
    }

    #[test]
    fn test_expand_template() {
        let mut vars = std::collections::HashMap::new();
        vars.insert("name".to_string(), "Alice".to_string());
        let result = expand_template("Hello {{name}}!", &vars);
        assert_eq!(result, "Hello Alice!");
    }
}
