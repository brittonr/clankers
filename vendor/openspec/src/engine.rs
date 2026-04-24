//! Main entry point for spec operations

use std::path::Path;
use std::path::PathBuf;

use crate::config::SpecConfig;
use crate::core::change;
use crate::core::merge;
use crate::core::spec;
use crate::core::verify;

/// Main entry point for spec operations
pub struct SpecEngine {
    pub openspec_dir: PathBuf,
}

impl SpecEngine {
    pub fn new(project_root: &Path) -> Self {
        Self {
            openspec_dir: project_root.join("openspec"),
        }
    }

    /// Check if openspec/ exists
    pub fn is_initialized(&self) -> bool {
        self.openspec_dir.is_dir()
    }

    /// Initialize openspec/ directory
    pub fn init(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.openspec_dir.join("specs"))?;
        std::fs::create_dir_all(self.openspec_dir.join("changes"))?;
        std::fs::create_dir_all(self.openspec_dir.join("schemas"))?;
        // Write default config
        std::fs::write(
            self.openspec_dir.join("config.yaml"),
            "schema: spec-driven\ncontext: |\n  # Add project context here\n",
        )?;
        Ok(())
    }

    /// Discover all specs
    pub fn discover_specs(&self) -> Vec<spec::Spec> {
        spec::scan_specs(&self.openspec_dir.join("specs"))
    }

    /// Discover active changes
    pub fn discover_changes(&self) -> Vec<change::ChangeInfo> {
        change::list_changes(&self.openspec_dir.join("changes"))
    }

    /// Create a new change
    pub fn create_change(&self, name: &str, schema_name: Option<&str>) -> std::io::Result<PathBuf> {
        let schema = schema_name.unwrap_or("spec-driven");
        change::create_change(&self.openspec_dir.join("changes"), name, schema)
    }

    /// Archive a change
    pub fn archive_change(&self, name: &str) -> std::io::Result<PathBuf> {
        change::archive_change(&self.openspec_dir.join("changes"), name)
    }

    /// Sync delta specs to main specs
    pub fn sync_change(&self, name: &str, dry_run: bool) -> merge::SyncResult {
        let change_specs = self.openspec_dir.join("changes").join(name).join("specs");
        let main_specs = self.openspec_dir.join("specs");
        merge::sync_change(&main_specs, &change_specs, dry_run)
    }

    /// Verify a change
    pub fn verify_change(&self, name: &str) -> verify::VerifyReport {
        let change_dir = self.openspec_dir.join("changes").join(name);
        verify::verify_basic(&change_dir)
    }

    /// Get relevant specs for agent context
    pub fn specs_for_context(&self) -> String {
        let specs = self.discover_specs();
        if specs.is_empty() {
            return String::new();
        }
        let mut context = String::from("## Project Specifications\n\n");
        for spec in &specs {
            context.push_str(&format!("### {} ({})", spec.domain, spec.file_path.display()));
            context.push('\n');
            if let Some(ref purpose) = spec.purpose {
                context.push_str(purpose);
                context.push('\n');
            }
            for req in &spec.requirements {
                context.push_str(&format!("- **{}** [{:?}]\n", req.heading, req.strength));
            }
            context.push('\n');
        }
        context
    }

    /// Load project config
    pub fn config(&self) -> SpecConfig {
        SpecConfig::load(&self.openspec_dir)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_spec_engine_new() {
        let dir = TempDir::new().expect("failed to create temp dir");
        let engine = SpecEngine::new(dir.path());

        assert_eq!(engine.openspec_dir, dir.path().join("openspec"));
    }

    #[test]
    fn test_is_initialized_false() {
        let dir = TempDir::new().expect("failed to create temp dir");
        let engine = SpecEngine::new(dir.path());

        assert!(!engine.is_initialized());
    }

    #[test]
    fn test_init() {
        let dir = TempDir::new().expect("failed to create temp dir");
        let engine = SpecEngine::new(dir.path());

        engine.init().expect("failed to initialize");

        assert!(engine.is_initialized());
        assert!(engine.openspec_dir.join("specs").is_dir());
        assert!(engine.openspec_dir.join("changes").is_dir());
        assert!(engine.openspec_dir.join("schemas").is_dir());
        assert!(engine.openspec_dir.join("config.yaml").exists());
    }

    #[test]
    fn test_discover_empty() {
        let dir = TempDir::new().expect("failed to create temp dir");
        let engine = SpecEngine::new(dir.path());

        engine.init().expect("failed to initialize");

        assert!(engine.discover_specs().is_empty());
        assert!(engine.discover_changes().is_empty());
    }

    #[test]
    fn test_specs_for_context_empty() {
        let dir = TempDir::new().expect("failed to create temp dir");
        let engine = SpecEngine::new(dir.path());

        engine.init().expect("failed to initialize");

        assert!(engine.specs_for_context().is_empty());
    }

    #[test]
    fn test_create_and_list_change() {
        let dir = TempDir::new().expect("failed to create temp dir");
        let engine = SpecEngine::new(dir.path());

        engine.init().expect("failed to initialize");
        engine.create_change("test-change", None).expect("failed to create change");

        let changes = engine.discover_changes();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].name, "test-change");
    }

    #[test]
    fn test_archive_change() {
        let dir = TempDir::new().expect("failed to create temp dir");
        let engine = SpecEngine::new(dir.path());

        engine.init().expect("failed to initialize");
        engine.create_change("old-change", None).expect("failed to create change");
        engine.archive_change("old-change").expect("failed to archive change");

        let changes = engine.discover_changes();
        assert!(changes.is_empty());
    }
}
