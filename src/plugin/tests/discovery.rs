use super::*;
use crate::plugin::manifest;

fn write_plugin_manifest(dir: &std::path::Path, name: &str, manifest: serde_json::Value) {
    let plugin_dir = dir.join(name);
    std::fs::create_dir_all(&plugin_dir).unwrap();
    std::fs::write(plugin_dir.join("plugin.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
}

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
fn manifest_stdio_launch_policy_parsed_and_validated() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plugin.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&serde_json::json!({
            "name": "stdio-test-plugin",
            "version": "0.1.0",
            "kind": "stdio",
            "permissions": ["net"],
            "stdio": {
                "command": "python3",
                "args": ["plugin.py", "--stdio"],
                "working_dir": "project-root",
                "env_allowlist": ["GITHUB_TOKEN", "FASTMAIL_TOKEN"],
                "sandbox": "restricted",
                "writable_roots": [".git", "build/output"],
                "allow_network": true
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let manifest = manifest::PluginManifest::load(&path).unwrap();
    manifest.validate().unwrap();

    assert_eq!(manifest.kind, manifest::PluginKind::Stdio);
    let stdio = manifest.stdio.as_ref().expect("stdio config");
    assert_eq!(stdio.command.as_deref(), Some("python3"));
    assert_eq!(stdio.args, vec!["plugin.py", "--stdio"]);
    assert_eq!(stdio.working_dir, Some(manifest::PluginWorkingDirectory::ProjectRoot));
    assert_eq!(stdio.env_allowlist, vec!["GITHUB_TOKEN", "FASTMAIL_TOKEN"]);
    assert_eq!(stdio.sandbox, Some(manifest::PluginSandboxMode::Restricted));
    assert_eq!(stdio.writable_roots, vec![".git", "build/output"]);
    assert!(stdio.allow_network);
}

#[test]
fn manifest_stdio_requires_launch_policy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plugin.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&serde_json::json!({
            "name": "stdio-no-policy",
            "version": "0.1.0",
            "kind": "stdio"
        }))
        .unwrap(),
    )
    .unwrap();

    let manifest = manifest::PluginManifest::load(&path).unwrap();
    assert_eq!(manifest.validate(), Err(manifest::ManifestValidationError::MissingStdioLaunchPolicy));
}

#[test]
fn manifest_stdio_rejects_blank_command() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plugin.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&serde_json::json!({
            "name": "stdio-blank-command",
            "version": "0.1.0",
            "kind": "stdio",
            "stdio": {
                "command": "   ",
                "sandbox": "inherit"
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let manifest = manifest::PluginManifest::load(&path).unwrap();
    assert_eq!(manifest.validate(), Err(manifest::ManifestValidationError::EmptyStdioCommand));
}

#[test]
fn manifest_stdio_requires_sandbox_mode() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plugin.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&serde_json::json!({
            "name": "stdio-no-sandbox",
            "version": "0.1.0",
            "kind": "stdio",
            "stdio": {
                "command": "python3"
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let manifest = manifest::PluginManifest::load(&path).unwrap();
    assert_eq!(manifest.validate(), Err(manifest::ManifestValidationError::MissingStdioSandbox));
}

#[test]
fn manifest_non_stdio_rejects_stdio_block() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plugin.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&serde_json::json!({
            "name": "extism-with-stdio-block",
            "version": "0.1.0",
            "kind": "extism",
            "wasm": "plugin.wasm",
            "stdio": {
                "command": "python3",
                "sandbox": "inherit"
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let manifest = manifest::PluginManifest::load(&path).unwrap();
    assert_eq!(manifest.validate(), Err(manifest::ManifestValidationError::UnexpectedStdioLaunchPolicy));
}

#[test]
fn discover_mixed_extism_and_stdio_plugins() {
    let dir = tempfile::tempdir().unwrap();
    write_plugin_manifest(
        dir.path(),
        "extism-plugin",
        serde_json::json!({
            "name": "extism-plugin",
            "version": "0.1.0",
            "kind": "extism",
            "wasm": "plugin.wasm"
        }),
    );
    write_plugin_manifest(
        dir.path(),
        "stdio-plugin",
        serde_json::json!({
            "name": "stdio-plugin",
            "version": "0.1.0",
            "kind": "stdio",
            "stdio": {
                "command": "python3",
                "args": ["plugin.py"],
                "sandbox": "inherit"
            }
        }),
    );

    let mut mgr = PluginManager::new(dir.path().to_path_buf(), None);
    mgr.discover();

    let extism = mgr.get("extism-plugin").expect("extism discovered");
    assert_eq!(extism.state, PluginState::Loaded);
    assert_eq!(extism.manifest.kind, manifest::PluginKind::Extism);

    let stdio = mgr.get("stdio-plugin").expect("stdio discovered");
    assert_eq!(stdio.state, PluginState::Loaded);
    assert_eq!(stdio.manifest.kind, manifest::PluginKind::Stdio);
    assert_eq!(stdio.manifest.stdio.as_ref().and_then(|cfg| cfg.command.as_deref()), Some("python3"));
}

#[test]
fn discover_invalid_stdio_manifest_marks_plugin_error() {
    let dir = tempfile::tempdir().unwrap();
    write_plugin_manifest(
        dir.path(),
        "valid-extism-plugin",
        serde_json::json!({
            "name": "valid-extism-plugin",
            "version": "0.1.0",
            "kind": "extism",
            "wasm": "plugin.wasm"
        }),
    );
    write_plugin_manifest(
        dir.path(),
        "invalid-stdio-plugin",
        serde_json::json!({
            "name": "invalid-stdio-plugin",
            "version": "0.1.0",
            "kind": "stdio",
            "stdio": {
                "args": ["plugin.py"],
                "sandbox": "inherit"
            }
        }),
    );

    let mut mgr = PluginManager::new(dir.path().to_path_buf(), None);
    mgr.discover();

    let valid = mgr.get("valid-extism-plugin").expect("valid plugin preserved");
    assert_eq!(valid.state, PluginState::Loaded);

    let invalid = mgr.get("invalid-stdio-plugin").expect("invalid plugin still listed");
    match &invalid.state {
        PluginState::Error(message) => assert!(message.contains("stdio.command"), "unexpected error: {message}"),
        other => panic!("expected error state, got {other:?}"),
    }
}

#[test]
fn manifest_zellij_kind_parses_and_validates() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plugin.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&serde_json::json!({
            "name": "zellij-plugin",
            "version": "0.1.0",
            "kind": "zellij"
        }))
        .unwrap(),
    )
    .unwrap();

    let manifest = manifest::PluginManifest::load(&path).unwrap();
    manifest.validate().unwrap();
    assert_eq!(manifest.kind, manifest::PluginKind::Zellij);
}

#[test]
fn init_plugin_manager_keeps_zellij_plugins_loaded_without_wasm_errors() {
    let dir = tempfile::tempdir().unwrap();
    write_plugin_manifest(
        dir.path(),
        "zellij-plugin",
        serde_json::json!({
            "name": "zellij-plugin",
            "version": "0.1.0",
            "kind": "zellij"
        }),
    );

    let manager = crate::modes::common::init_plugin_manager(dir.path(), None, &[]);
    let mgr = manager.lock().unwrap_or_else(|e| e.into_inner());
    let info = mgr.get("zellij-plugin").expect("zellij plugin discovered");
    assert_eq!(info.state, PluginState::Loaded);
    assert_eq!(info.manifest.kind, manifest::PluginKind::Zellij);
}

#[test]
fn init_plugin_manager_skips_wasm_load_for_stdio_manifests() {
    let dir = tempfile::tempdir().unwrap();
    write_plugin_manifest(
        dir.path(),
        "stdio-init-test-plugin",
        serde_json::json!({
            "name": "stdio-init-test-plugin",
            "version": "0.1.0",
            "kind": "stdio",
            "stdio": {
                "command": "python3",
                "args": ["plugin.py"],
                "sandbox": "inherit"
            }
        }),
    );

    let manager = crate::modes::common::init_plugin_manager(dir.path(), None, &[]);
    let mgr = manager.lock().unwrap_or_else(|e| e.into_inner());
    let info = mgr.get("stdio-init-test-plugin").expect("stdio plugin discovered");
    assert_eq!(info.state, PluginState::Loaded);
    assert_eq!(info.manifest.kind, manifest::PluginKind::Stdio);
}

#[tokio::test]
async fn daemon_mode_plugin_init_handles_empty_dirs_without_plugins() {
    let dir = tempfile::tempdir().unwrap();
    let manager = crate::plugin::tests::stdio_runtime::init_manager_with_restart_delays(
        dir.path(),
        crate::plugin::PluginRuntimeMode::Daemon,
        "5,10,15,20,25",
    );
    let mgr = manager.lock().unwrap_or_else(|e| e.into_inner());
    assert!(mgr.is_empty());
    drop(mgr);
    assert!(crate::plugin::build_protocol_plugin_summaries(&manager).is_empty());
}

#[tokio::test]
async fn daemon_mode_plugin_init_continues_when_some_plugins_fail() {
    let dir = tempfile::tempdir().unwrap();
    write_plugin_manifest(
        dir.path(),
        "broken-extism-plugin",
        serde_json::json!({
            "name": "broken-extism-plugin",
            "version": "0.1.0",
            "kind": "extism",
            "wasm": "missing.wasm"
        }),
    );
    crate::plugin::tests::stdio_runtime::write_stdio_plugin_manifest(
        dir.path(),
        "healthy-stdio-plugin",
        "ready_register",
        "daemon",
        None,
        None,
    );

    let manager = crate::plugin::tests::stdio_runtime::init_manager_with_restart_delays(
        dir.path(),
        crate::plugin::PluginRuntimeMode::Daemon,
        "5,10,15,20,25",
    );
    crate::plugin::tests::stdio_runtime::wait_for_plugin_state(
        &manager,
        "healthy-stdio-plugin",
        std::time::Duration::from_secs(2),
        |state| matches!(state, PluginState::Active),
    )
    .await;
    crate::plugin::tests::stdio_runtime::wait_for_live_tool(
        &manager,
        "healthy-stdio-plugin",
        "healthy_stdio_plugin_tool",
        std::time::Duration::from_secs(2),
    )
    .await;

    let mgr = manager.lock().unwrap_or_else(|e| e.into_inner());
    let broken = mgr.get("broken-extism-plugin").expect("broken extism plugin discovered");
    assert_eq!(broken.state.summary_label(), "Error");
    let healthy = mgr.get("healthy-stdio-plugin").expect("healthy stdio plugin discovered");
    assert_eq!(healthy.state, PluginState::Active);
    drop(mgr);

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[test]
fn protocol_plugin_summaries_include_facade_runtime_metadata() {
    let dir = tempfile::tempdir().unwrap();
    write_plugin_manifest(
        dir.path(),
        "valid-extism-plugin",
        serde_json::json!({
            "name": "valid-extism-plugin",
            "version": "0.1.0",
            "kind": "extism",
            "wasm": "plugin.wasm"
        }),
    );
    write_plugin_manifest(
        dir.path(),
        "invalid-stdio-plugin",
        serde_json::json!({
            "name": "invalid-stdio-plugin",
            "version": "0.1.0",
            "kind": "stdio",
            "stdio": {
                "args": ["plugin.py"],
                "sandbox": "inherit"
            }
        }),
    );

    let manager = crate::modes::common::init_plugin_manager(dir.path(), None, &[]);
    let summaries = crate::plugin::build_protocol_plugin_summaries(&manager);

    let extism = summaries.iter().find(|summary| summary.name == "valid-extism-plugin").unwrap();
    assert_eq!(extism.kind.as_deref(), Some("extism"));
    assert_eq!(extism.state, "Error");
    assert!(extism.last_error.as_deref().is_some_and(|error| error.contains("WASM file not found")));

    let stdio = summaries.iter().find(|summary| summary.name == "invalid-stdio-plugin").unwrap();
    assert_eq!(stdio.kind.as_deref(), Some("stdio"));
    assert_eq!(stdio.state, "Error");
    assert!(stdio.last_error.as_deref().is_some_and(|error| error.contains("stdio.command")));
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
