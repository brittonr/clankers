//! Agent definition format (markdown + YAML frontmatter)

use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

/// Source of an agent definition
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentSource {
    /// From ~/.clankers/agent/agents/
    User,
    /// From .clankers/agents/ in project
    Project,
}

/// Scope for agent discovery
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum AgentScope {
    /// Only user-level agents
    #[default]
    User,
    /// Only project-level agents
    Project,
    /// Both user and project agents
    Both,
}

/// A parsed agent definition from a markdown file with YAML frontmatter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    pub system_prompt: String,
    pub source: AgentSource,
    pub file_path: PathBuf,
}

/// Parse an agent definition from a markdown file.
///
/// Format:
/// ```markdown
/// ---
/// name: scout
/// description: Fast read-only recon agent
/// tools: read, grep, find, ls, bash
/// model: claude-haiku-4-5
/// ---
///
/// You are a scout agent...
/// ```
pub fn parse_agent_file(path: &Path, source: AgentSource) -> Result<AgentConfig, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    parse_agent_content(&content, path, source)
}

/// Parse agent definition from content string
pub fn parse_agent_content(content: &str, path: &Path, source: AgentSource) -> Result<AgentConfig, String> {
    // Split on --- delimiters
    let parts: Vec<&str> = content.split("---").collect();

    if parts.len() < 3 {
        return Err(format!("{}: Missing frontmatter delimiters (expected '---' ... '---')", path.display()));
    }

    // parts[0] is empty or whitespace before first ---
    // parts[1] is the frontmatter
    // parts[2..] is the system prompt body
    let frontmatter = parts[1].trim();
    let system_prompt = parts[2..].join("---").trim().to_string();

    if system_prompt.is_empty() {
        return Err(format!("{}: Empty system prompt", path.display()));
    }

    // Parse frontmatter manually (simple key: value format)
    let mut name = None;
    let mut description = None;
    let mut model = None;
    let mut tools_str = None;

    for line in frontmatter.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();

            match key {
                "name" => name = Some(value.to_string()),
                "description" => description = Some(value.to_string()),
                "model" => model = Some(value.to_string()),
                "tools" => tools_str = Some(value.to_string()),
                _ => {
                    // Ignore unknown keys
                }
            }
        }
    }

    let name = name.ok_or_else(|| format!("{}: Missing 'name' in frontmatter", path.display()))?;
    let description = description.ok_or_else(|| format!("{}: Missing 'description' in frontmatter", path.display()))?;

    // Parse tools from comma-separated string
    let tools =
        tools_str.map(|s| s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect::<Vec<_>>());

    Ok(AgentConfig {
        name,
        description,
        model,
        tools,
        system_prompt,
        source,
        file_path: path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_parse_complete_agent() {
        let content = r#"---
name: scout
description: Fast read-only recon agent
model: claude-haiku-4-5
tools: read, grep, find, ls, bash
---

You are a scout agent for quick reconnaissance."#;

        let result = parse_agent_content(content, Path::new("test.md"), AgentSource::User);
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.name, "scout");
        assert_eq!(config.description, "Fast read-only recon agent");
        assert_eq!(config.model, Some("claude-haiku-4-5".to_string()));
        assert_eq!(
            config.tools,
            Some(vec![
                "read".to_string(),
                "grep".to_string(),
                "find".to_string(),
                "ls".to_string(),
                "bash".to_string(),
            ])
        );
        assert_eq!(config.system_prompt, "You are a scout agent for quick reconnaissance.");
        assert_eq!(config.source, AgentSource::User);
    }

    #[test]
    fn test_parse_minimal_agent() {
        let content = r#"---
name: minimal
description: Minimal agent
---

System prompt here."#;

        let result = parse_agent_content(content, Path::new("test.md"), AgentSource::Project);
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.name, "minimal");
        assert_eq!(config.description, "Minimal agent");
        assert!(config.model.is_none());
        assert!(config.tools.is_none());
        assert_eq!(config.system_prompt, "System prompt here.");
    }

    #[test]
    fn test_parse_missing_frontmatter() {
        let content = "Just a prompt without frontmatter";
        let result = parse_agent_content(content, Path::new("test.md"), AgentSource::User);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing frontmatter"));
    }

    #[test]
    fn test_parse_missing_name() {
        let content = r#"---
description: No name
---

Prompt"#;

        let result = parse_agent_content(content, Path::new("test.md"), AgentSource::User);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'name'"));
    }

    #[test]
    fn test_parse_missing_description() {
        let content = r#"---
name: test
---

Prompt"#;

        let result = parse_agent_content(content, Path::new("test.md"), AgentSource::User);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'description'"));
    }

    #[test]
    fn test_parse_empty_system_prompt() {
        let content = r#"---
name: test
description: Test agent
---

"#;

        let result = parse_agent_content(content, Path::new("test.md"), AgentSource::User);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Empty system prompt"));
    }

    #[test]
    fn test_parse_tools_with_spaces() {
        let content = r#"---
name: test
description: Test
tools: read,  grep,  find , bash
---

Prompt"#;

        let result = parse_agent_content(content, Path::new("test.md"), AgentSource::User);
        assert!(result.is_ok());

        let config = result.unwrap();
        let tools = config.tools.unwrap();
        assert_eq!(tools, vec!["read", "grep", "find", "bash"]);
    }

    #[test]
    fn test_parse_multiline_prompt() {
        let content = r#"---
name: test
description: Test
---

Line 1
Line 2
Line 3"#;

        let result = parse_agent_content(content, Path::new("test.md"), AgentSource::User);
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.system_prompt, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_parse_prompt_with_triple_dash() {
        let content = r#"---
name: test
description: Test
---

Prompt with --- in it should work"#;

        let result = parse_agent_content(content, Path::new("test.md"), AgentSource::User);
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.system_prompt, "Prompt with --- in it should work");
    }

    #[test]
    fn test_parse_ignores_comments() {
        let content = r#"---
name: test
# This is a comment
description: Test
# Another comment
---

Prompt"#;

        let result = parse_agent_content(content, Path::new("test.md"), AgentSource::User);
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.name, "test");
        assert_eq!(config.description, "Test");
    }

    #[test]
    fn test_agent_source_serialization() {
        let user = AgentSource::User;
        let project = AgentSource::Project;

        let user_json = serde_json::to_string(&user).unwrap();
        let project_json = serde_json::to_string(&project).unwrap();

        assert_eq!(user_json, "\"user\"");
        assert_eq!(project_json, "\"project\"");
    }

    #[test]
    fn test_agent_scope_default() {
        let scope = AgentScope::default();
        assert_eq!(scope, AgentScope::User);
    }

    #[test]
    fn test_parse_file_path_stored() {
        let content = r#"---
name: test
description: Test
---

Prompt"#;

        let path = PathBuf::from("/test/path/agent.md");
        let result = parse_agent_content(content, &path, AgentSource::User);
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.file_path, path);
    }
}
