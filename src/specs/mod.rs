//! Spec-driven development (OpenSpec)

pub mod artifact;
pub mod change;
pub mod config;
pub mod delta;
pub mod merge;
pub mod schema;
pub mod spec;
pub mod templates;
pub mod verify;

use std::path::Path;
use std::path::PathBuf;

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
    pub fn config(&self) -> config::SpecConfig {
        config::SpecConfig::load(&self.openspec_dir)
    }
}
