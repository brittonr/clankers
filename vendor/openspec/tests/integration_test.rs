use openspec::core::change;
use openspec::core::spec;
use openspec::core::templates;
use openspec::core::verify;

#[test]
fn test_pure_api_basic() {
    // Test spec parsing
    let content = "## Purpose\nTest spec\n\n## Feature\nThe system MUST work.";
    let spec = spec::parse_spec_content(content, "test").unwrap();
    assert_eq!(spec.domain, "test");
    assert_eq!(spec.requirements.len(), 1);
    assert_eq!(spec.requirements[0].heading, "Feature");

    // Test strength detection
    let strength = spec::detect_strength("The system MUST work.");
    assert_eq!(strength, spec::RequirementStrength::Must);

    // Test task parsing
    let tasks = "- [x] Done\n- [ ] Todo";
    let progress = change::parse_task_progress_content(tasks).unwrap();
    assert_eq!(progress.done, 1);
    assert_eq!(progress.todo, 1);

    // Test template expansion
    let expanded =
        templates::expand_template("# {{change_name}}\nContext: {{context}}", "test-change", "test context", &[]);
    assert!(expanded.contains("test-change"));
    assert!(expanded.contains("test context"));

    // Test verification
    let report = verify::verify_from_content(Some(tasks), true);
    assert!(!report.has_critical());
}

#[cfg(feature = "fs")]
#[test]
fn test_fs_api_basic() {
    use openspec::SpecEngine;
    use tempfile::TempDir;

    let dir = TempDir::new().expect("failed to create temp dir");
    let engine = SpecEngine::new(dir.path());

    // Test initialization
    assert!(!engine.is_initialized());
    engine.init().expect("failed to init");
    assert!(engine.is_initialized());

    // Test empty discovery
    assert!(engine.discover_specs().is_empty());
    assert!(engine.discover_changes().is_empty());

    // Test change creation
    engine.create_change("test-change", None).expect("failed to create change");
    let changes = engine.discover_changes();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].name, "test-change");
}
