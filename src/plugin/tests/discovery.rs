use super::*;
use crate::plugin::manifest;

// ── Discovery tests ──────────────────────────────────────────────

#[test]
fn discover_finds_test_plugin() {
    let mgr = manager_with_test_plugin();
    assert!(mgr.get("clankers-test-plugin").is_some(), "Test plugin should be discovered");
}

#[test]
fn discover_reads_manifest_metadata() {
    let mgr = manager_with_test_plugin();
    let info = mgr.get("clankers-test-plugin").unwrap();
    assert_eq!(info.name, "clankers-test-plugin");
    assert_eq!(info.version, "0.1.0");
    assert_eq!(info.state, PluginState::Loaded);
    assert_eq!(info.manifest.description, "Test plugin for exercising the clankers WASM plugin system");
}

#[test]
fn discover_reads_manifest_tools_and_commands() {
    let mgr = manager_with_test_plugin();
    let info = mgr.get("clankers-test-plugin").unwrap();
    assert_eq!(info.manifest.tools, vec!["test_echo", "test_reverse"]);
    assert_eq!(info.manifest.commands, vec!["test"]);
}

#[test]
fn discover_reads_manifest_permissions() {
    let mgr = manager_with_test_plugin();
    let info = mgr.get("clankers-test-plugin").unwrap();
    assert_eq!(info.manifest.permissions, vec!["fs:read"]);
}

#[test]
fn discover_reads_manifest_events() {
    let mgr = manager_with_test_plugin();
    let info = mgr.get("clankers-test-plugin").unwrap();
    assert_eq!(info.manifest.events, vec!["agent_start", "agent_end", "tool_call"]);
}

#[test]
fn discover_empty_dir_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = PluginManager::new(dir.path().to_path_buf(), None);
    assert!(mgr.is_empty());
    assert_eq!(mgr.len(), 0);
}

#[test]
fn discover_nonexistent_dir_is_empty() {
    let mgr = PluginManager::new(PathBuf::from("/tmp/clankers-nonexistent-dir-abc"), None);
    assert!(mgr.is_empty());
}

#[test]
fn list_returns_all_discovered() {
    let mgr = manager_with_test_plugin();
    let list = mgr.list();
    assert!(list.iter().any(|p| p.name == "clankers-test-plugin"));
}

#[test]
fn discover_from_project_dir() {
    let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let empty_global = tempfile::tempdir().unwrap();
    let mut mgr = PluginManager::new(empty_global.path().to_path_buf(), Some(plugins_dir));
    mgr.discover();
    assert!(mgr.get("clankers-test-plugin").is_some());
}

// ── Manifest loading unit tests ──────────────────────────────────

#[test]
fn manifest_load_from_file() {
    let manifest_path = test_plugin_dir().join("plugin.json");
    let m = manifest::PluginManifest::load(&manifest_path).unwrap();
    assert_eq!(m.name, "clankers-test-plugin");
    assert_eq!(m.wasm.as_deref(), Some("clankers_test_plugin.wasm"));
    assert!(matches!(m.kind, manifest::PluginKind::Extism));
}

#[test]
fn manifest_tool_definitions_parsed() {
    let manifest_path = test_plugin_dir().join("plugin.json");
    let m = manifest::PluginManifest::load(&manifest_path).unwrap();
    assert_eq!(m.tool_definitions.len(), 2);

    let echo = &m.tool_definitions[0];
    assert_eq!(echo.name, "test_echo");
    assert_eq!(echo.handler, "handle_tool_call");
    assert!(echo.description.contains("Echo"));
    let schema = &echo.input_schema;
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["text"].is_object());

    let reverse = &m.tool_definitions[1];
    assert_eq!(reverse.name, "test_reverse");
    assert!(reverse.description.contains("Reverse"));
}

#[test]
fn manifest_tool_definitions_default_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plugin.json");
    std::fs::write(&path, r#"{"name":"minimal","version":"0.1.0"}"#).ok();
    let m = manifest::PluginManifest::load(&path).unwrap();
    assert!(m.tool_definitions.is_empty());
}

#[test]
fn manifest_load_nonexistent_returns_none() {
    let result = manifest::PluginManifest::load(std::path::Path::new("/tmp/no-such-file.json"));
    assert!(result.is_none());
}

#[test]
fn manifest_load_invalid_json_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plugin.json");
    std::fs::write(&path, "not valid json{{{").ok();
    assert!(manifest::PluginManifest::load(&path).is_none());
}
