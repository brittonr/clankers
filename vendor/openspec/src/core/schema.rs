//! Schema loading (YAML)

#[cfg(feature = "fs")]
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

/// Schema artifact definition (from schema.yaml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaArtifact {
    pub id: String,
    pub generates: String,
    #[serde(default)]
    pub requires: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub artifacts: Vec<SchemaArtifact>,
}

/// Built-in spec-driven schema (pure function)
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

#[cfg(feature = "fs")]
/// Load a schema from a YAML file
pub fn load_schema(path: &Path) -> Option<Schema> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_yaml::from_str(&content).ok()
}

#[cfg(feature = "fs")]
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
    use super::*;

    #[test]
    fn test_builtin_spec_driven() {
        let schema = builtin_spec_driven();
        assert_eq!(schema.name, "spec-driven");
        assert!(!schema.artifacts.is_empty());
        assert!(schema.artifacts.iter().any(|a| a.id == "proposal"));
    }

    #[cfg(all(test, feature = "fs"))]
    mod fs_tests {
        use super::*;
        use tempfile::TempDir;

        #[test]
        fn test_load_schema_yaml() {
            let dir = TempDir::new().expect("failed to create temp dir");
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
            std::fs::write(&schema_file, yaml).expect("failed to write schema file");

            let schema = load_schema(&schema_file).expect("failed to load schema");
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
            let dir = TempDir::new().expect("failed to create temp dir");
            let schema_dir = dir.path().join("custom");
            std::fs::create_dir(&schema_dir).expect("failed to create schema dir");
            let schema_file = schema_dir.join("schema.yaml");
            std::fs::write(&schema_file, "name: custom\nartifacts: []")
                .expect("failed to write schema file");

            let schema = resolve_schema(Some("custom"), None, Some(dir.path()));
            assert_eq!(schema.name, "custom");
        }

        #[test]
        fn test_resolve_schema_project_overrides_user() {
            let user_dir = TempDir::new().expect("failed to create user temp dir");
            let project_dir = TempDir::new().expect("failed to create project temp dir");

            let user_schema = user_dir.path().join("test");
            std::fs::create_dir(&user_schema).expect("failed to create user schema dir");
            std::fs::write(
                user_schema.join("schema.yaml"),
                "name: user-test\nartifacts: []",
            )
            .expect("failed to write user schema");

            let project_schema = project_dir.path().join("test");
            std::fs::create_dir(&project_schema).expect("failed to create project schema dir");
            std::fs::write(
                project_schema.join("schema.yaml"),
                "name: project-test\nartifacts: []",
            )
            .expect("failed to write project schema");

            let schema = resolve_schema(
                Some("test"),
                Some(project_dir.path()),
                Some(user_dir.path()),
            );
            assert_eq!(schema.name, "project-test");
        }
    }
}
