//! Schema loading (YAML)

use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

use super::artifact::SchemaArtifact;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub artifacts: Vec<SchemaArtifact>,
}

/// Load a schema from a YAML file
pub fn load_schema(path: &Path) -> Option<Schema> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_yaml::from_str(&content).ok()
}

/// Built-in spec-driven schema
pub fn builtin_spec_driven() -> Schema {
    Schema {
        name: "spec-driven".to_string(),
        artifacts: vec![
            SchemaArtifact {
                id: "proposal".to_string(),
                generates: "proposal.md".to_string(),
                requires: vec![],
            },
            SchemaArtifact {
                id: "specs".to_string(),
                generates: "specs/**/*.md".to_string(),
                requires: vec!["proposal".to_string()],
            },
            SchemaArtifact {
                id: "design".to_string(),
                generates: "design.md".to_string(),
                requires: vec!["proposal".to_string()],
            },
            SchemaArtifact {
                id: "tasks".to_string(),
                generates: "tasks.md".to_string(),
                requires: vec!["specs".to_string(), "design".to_string()],
            },
        ],
    }
}

/// Resolve schema: CLI flag > change metadata > project config > default
pub fn resolve_schema(
    schema_name: Option<&str>,
    project_schemas_dir: Option<&Path>,
    user_schemas_dir: Option<&Path>,
) -> Schema {
    let name = schema_name.unwrap_or("spec-driven");

    // Try project schemas
    if let Some(dir) = project_schemas_dir {
        let path = dir.join(name).join("schema.yaml");
        if let Some(schema) = load_schema(&path) {
            return schema;
        }
    }

    // Try user schemas
    if let Some(dir) = user_schemas_dir {
        let path = dir.join(name).join("schema.yaml");
        if let Some(schema) = load_schema(&path) {
            return schema;
        }
    }

    // Default
    builtin_spec_driven()
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_builtin_spec_driven() {
        let schema = builtin_spec_driven();
        assert_eq!(schema.name, "spec-driven");
        assert!(!schema.artifacts.is_empty());
        assert!(schema.artifacts.iter().any(|a| a.id == "proposal"));
    }

    #[test]
    fn test_load_schema_yaml() {
        let dir = TempDir::new().unwrap();
        let schema_file = dir.path().join("schema.yaml");
        let yaml = r#"
name: test-schema
artifacts:
  - id: first
    generates: first.md
    requires: []
  - id: second
    generates: second.md
    requires: [first]
"#;
        std::fs::write(&schema_file, yaml).unwrap();

        let schema = load_schema(&schema_file).unwrap();
        assert_eq!(schema.name, "test-schema");
        assert_eq!(schema.artifacts.len(), 2);
        assert_eq!(schema.artifacts[0].id, "first");
        assert_eq!(schema.artifacts[1].id, "second");
        assert_eq!(schema.artifacts[1].requires, vec!["first"]);
    }

    #[test]
    fn test_load_schema_missing_file() {
        let result = load_schema(std::path::Path::new("/nonexistent/schema.yaml"));
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_schema_default() {
        let schema = resolve_schema(None, None, None);
        assert_eq!(schema.name, "spec-driven");
    }

    #[test]
    fn test_resolve_schema_from_user_dir() {
        let dir = TempDir::new().unwrap();
        let schema_dir = dir.path().join("custom");
        std::fs::create_dir(&schema_dir).unwrap();
        let schema_file = schema_dir.join("schema.yaml");
        std::fs::write(&schema_file, "name: custom\nartifacts: []").unwrap();

        let schema = resolve_schema(Some("custom"), None, Some(dir.path()));
        assert_eq!(schema.name, "custom");
    }

    #[test]
    fn test_resolve_schema_project_overrides_user() {
        let user_dir = TempDir::new().unwrap();
        let project_dir = TempDir::new().unwrap();

        let user_schema = user_dir.path().join("test");
        std::fs::create_dir(&user_schema).unwrap();
        std::fs::write(user_schema.join("schema.yaml"), "name: user-test\nartifacts: []").unwrap();

        let project_schema = project_dir.path().join("test");
        std::fs::create_dir(&project_schema).unwrap();
        std::fs::write(project_schema.join("schema.yaml"), "name: project-test\nartifacts: []").unwrap();

        let schema = resolve_schema(Some("test"), Some(project_dir.path()), Some(user_dir.path()));
        assert_eq!(schema.name, "project-test");
    }
}
