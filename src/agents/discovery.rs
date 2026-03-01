//! Agent discovery from user and project directories

use std::path::Path;

use super::AgentRegistry;
use super::definition::AgentScope;
use super::definition::AgentSource;
use super::definition::parse_agent_file;

/// Discover agent definitions from user and project directories.
pub fn discover_agents(
    global_agents_dir: &Path,
    project_agents_dir: Option<&Path>,
    scope: &AgentScope,
) -> AgentRegistry {
    let mut registry = AgentRegistry::new();

    // Always load user agents unless scope is Project-only
    if !matches!(scope, AgentScope::Project) && global_agents_dir.is_dir() {
        load_agents_from_dir(global_agents_dir, AgentSource::User, &mut registry);
    }

    // Load project agents only if scope allows
    if matches!(scope, AgentScope::Project | AgentScope::Both)
        && let Some(dir) = project_agents_dir
        && dir.is_dir()
    {
        load_agents_from_dir(dir, AgentSource::Project, &mut registry);
    }

    registry
}

/// Load all *.md files from a directory and register them
fn load_agents_from_dir(dir: &Path, source: AgentSource, registry: &mut AgentRegistry) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return, // Directory doesn't exist or can't be read
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Only process .md files
        if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        match parse_agent_file(&path, source.clone()) {
            Ok(config) => {
                registry.register(config);
            }
            Err(e) => {
                eprintln!("Warning: Failed to parse agent {}: {}", path.display(), e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_discover_user_agents() {
        let temp = TempDir::new().unwrap();
        let agents_dir = temp.path().join("agents");
        std::fs::create_dir(&agents_dir).unwrap();

        // Create a valid agent file
        let agent_file = agents_dir.join("test.md");
        let mut f = std::fs::File::create(&agent_file).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "name: test").unwrap();
        writeln!(f, "description: Test agent").unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "Test system prompt").unwrap();

        let registry = discover_agents(&agents_dir, None, &AgentScope::User);
        assert_eq!(registry.len(), 1);
        assert!(registry.get("test").is_some());
    }

    #[test]
    fn test_discover_project_agents() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path().join("project");
        std::fs::create_dir(&project_dir).unwrap();

        let agent_file = project_dir.join("proj.md");
        let mut f = std::fs::File::create(&agent_file).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "name: proj").unwrap();
        writeln!(f, "description: Project agent").unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "Project prompt").unwrap();

        let registry = discover_agents(temp.path(), Some(&project_dir), &AgentScope::Project);
        assert_eq!(registry.len(), 1);
        assert!(registry.get("proj").is_some());
    }

    #[test]
    fn test_scope_user_only() {
        let temp = TempDir::new().unwrap();
        let user_dir = temp.path().join("user");
        let project_dir = temp.path().join("project");
        std::fs::create_dir(&user_dir).unwrap();
        std::fs::create_dir(&project_dir).unwrap();

        // Create user agent
        let mut f = std::fs::File::create(user_dir.join("user.md")).unwrap();
        writeln!(f, "---\nname: user\ndescription: User\n---\nPrompt").unwrap();

        // Create project agent
        let mut f = std::fs::File::create(project_dir.join("proj.md")).unwrap();
        writeln!(f, "---\nname: proj\ndescription: Proj\n---\nPrompt").unwrap();

        let registry = discover_agents(&user_dir, Some(&project_dir), &AgentScope::User);
        assert_eq!(registry.len(), 1);
        assert!(registry.get("user").is_some());
        assert!(registry.get("proj").is_none());
    }
}
