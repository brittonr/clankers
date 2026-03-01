//! Plugin system (Extism WASM)

pub mod bridge;
pub mod host;
pub mod manifest;
pub mod registry;
pub mod sandbox;
pub mod ui;

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

/// Plugin state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    Loaded,
    Active,
    Error(String),
    Disabled,
}

/// A loaded plugin (metadata)
#[derive(Debug)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub state: PluginState,
    pub manifest: manifest::PluginManifest,
    pub path: PathBuf,
}

/// Plugin manager with WASM execution
pub struct PluginManager {
    plugins: HashMap<String, PluginInfo>,
    /// Loaded WASM plugin instances (behind Mutex because extism::Plugin is not Send)
    instances: HashMap<String, Mutex<extism::Plugin>>,
    global_dir: PathBuf,
    project_dir: Option<PathBuf>,
    /// Additional directories to scan for plugins
    extra_dirs: Vec<PathBuf>,
}

impl PluginManager {
    pub fn new(global_dir: PathBuf, project_dir: Option<PathBuf>) -> Self {
        Self {
            plugins: HashMap::new(),
            instances: HashMap::new(),
            global_dir,
            project_dir,
            extra_dirs: Vec::new(),
        }
    }

    /// Add an extra directory to scan for plugins
    pub fn add_plugin_dir(&mut self, dir: PathBuf) {
        self.extra_dirs.push(dir);
    }

    /// Discover plugins from directories
    pub fn discover(&mut self) {
        load_plugins_from_dir(&self.global_dir, &mut self.plugins);
        if let Some(ref dir) = self.project_dir {
            load_plugins_from_dir(dir, &mut self.plugins);
        }
        for dir in &self.extra_dirs {
            load_plugins_from_dir(dir, &mut self.plugins);
        }
    }

    /// Load a discovered plugin's WASM module
    pub fn load_wasm(&mut self, name: &str) -> Result<(), String> {
        let info = self.plugins.get_mut(name).ok_or_else(|| format!("Plugin '{}' not found", name))?;

        let wasm_filename = info.manifest.wasm.as_deref().unwrap_or("plugin.wasm");
        let wasm_path = info.path.join(wasm_filename);

        if !wasm_path.is_file() {
            info.state = PluginState::Error(format!("WASM file not found: {}", wasm_path.display()));
            return Err(format!("WASM file not found: {}", wasm_path.display()));
        }

        let manifest = extism::Manifest::new([extism::Wasm::file(&wasm_path)]);
        match extism::Plugin::new(manifest, [], true) {
            Ok(plugin) => {
                self.instances.insert(name.to_string(), Mutex::new(plugin));
                if let Some(info) = self.plugins.get_mut(name) {
                    info.state = PluginState::Active;
                }
                Ok(())
            }
            Err(e) => {
                let err_msg = format!("Failed to load WASM: {}", e);
                if let Some(info) = self.plugins.get_mut(name) {
                    info.state = PluginState::Error(err_msg.clone());
                }
                Err(err_msg)
            }
        }
    }

    /// Call a function on a loaded plugin
    pub fn call_plugin(&self, name: &str, function: &str, input: &str) -> Result<String, String> {
        let instance = self.instances.get(name).ok_or_else(|| format!("Plugin '{}' not loaded", name))?;

        let mut plugin = instance.lock().map_err(|e| format!("Plugin lock error: {}", e))?;

        let result = plugin.call::<&str, String>(function, input).map_err(|e| format!("Plugin call error: {}", e))?;

        Ok(result)
    }

    /// Check if a plugin has a specific function
    pub fn has_function(&self, name: &str, function: &str) -> bool {
        self.instances.get(name).and_then(|i| i.lock().ok()).is_some_and(|p| p.function_exists(function))
    }

    /// Get a loaded plugin's info
    pub fn get(&self, name: &str) -> Option<&PluginInfo> {
        self.plugins.get(name)
    }

    /// List all plugins
    pub fn list(&self) -> Vec<&PluginInfo> {
        self.plugins.values().collect()
    }

    /// Number of loaded plugins
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Reload a plugin
    pub fn reload(&mut self, name: &str) -> Result<(), String> {
        self.instances.remove(name);
        self.load_wasm(name)
    }

    /// Reload all plugins
    pub fn reload_all(&mut self) {
        let names: Vec<String> = self.plugins.keys().cloned().collect();
        for name in names {
            let _ = self.reload(&name);
        }
    }
}

fn load_plugins_from_dir(dir: &Path, plugins: &mut HashMap<String, PluginInfo>) {
    if !dir.is_dir() {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let manifest_path = path.join("plugin.json");
        if !manifest_path.is_file() {
            continue;
        }
        if let Some(manifest) = manifest::PluginManifest::load(&manifest_path) {
            let name = manifest.name.clone();
            plugins.insert(name.clone(), PluginInfo {
                name,
                version: manifest.version.clone(),
                state: PluginState::Loaded,
                manifest,
                path,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Path to the pre-built test plugin directory.
    /// The WASM must be built first via `plugins/clankers-test-plugin/build.sh`.
    fn test_plugin_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins/clankers-test-plugin")
    }

    /// Helper: create a PluginManager pointing at the test plugin directory.
    fn manager_with_test_plugin() -> PluginManager {
        let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
        let mut mgr = PluginManager::new(plugins_dir, None);
        mgr.discover();
        mgr
    }

    /// Helper: create a manager and load the test plugin WASM.
    fn loaded_manager() -> PluginManager {
        let mut mgr = manager_with_test_plugin();
        mgr.load_wasm("clankers-test-plugin").expect("Failed to load test plugin WASM");
        mgr
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

    // ── WASM loading tests ───────────────────────────────────────────

    #[test]
    fn load_wasm_transitions_to_active() {
        let mgr = loaded_manager();
        let info = mgr.get("clankers-test-plugin").unwrap();
        assert_eq!(info.state, PluginState::Active);
    }

    #[test]
    fn load_wasm_unknown_plugin_errors() {
        let mut mgr = manager_with_test_plugin();
        let result = mgr.load_wasm("nonexistent-plugin");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn load_wasm_missing_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let plugin_dir = dir.path().join("broken-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.json"), r#"{"name":"broken-plugin","version":"0.1.0"}"#).unwrap();

        let mut mgr = PluginManager::new(dir.path().to_path_buf(), None);
        mgr.discover();
        let result = mgr.load_wasm("broken-plugin");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("WASM file not found"));
    }

    // ── Function existence checks ────────────────────────────────────

    #[test]
    fn has_function_exported_functions() {
        let mgr = loaded_manager();
        assert!(mgr.has_function("clankers-test-plugin", "echo"));
        assert!(mgr.has_function("clankers-test-plugin", "greet"));
        assert!(mgr.has_function("clankers-test-plugin", "reverse"));
        assert!(mgr.has_function("clankers-test-plugin", "length"));
        assert!(mgr.has_function("clankers-test-plugin", "handle_tool_call"));
        assert!(mgr.has_function("clankers-test-plugin", "on_event"));
        assert!(mgr.has_function("clankers-test-plugin", "describe"));
        assert!(mgr.has_function("clankers-test-plugin", "count_words"));
        assert!(mgr.has_function("clankers-test-plugin", "uppercase"));
        assert!(mgr.has_function("clankers-test-plugin", "fail"));
        assert!(mgr.has_function("clankers-test-plugin", "parse_json"));
    }

    #[test]
    fn has_function_nonexistent_returns_false() {
        let mgr = loaded_manager();
        assert!(!mgr.has_function("clankers-test-plugin", "nonexistent_function"));
        assert!(!mgr.has_function("clankers-test-plugin", ""));
    }

    #[test]
    fn has_function_unloaded_plugin_returns_false() {
        let mgr = manager_with_test_plugin();
        assert!(!mgr.has_function("clankers-test-plugin", "echo"));
    }

    // ── String function calls ────────────────────────────────────────

    #[test]
    fn call_echo() {
        let mgr = loaded_manager();
        let result = mgr.call_plugin("clankers-test-plugin", "echo", "hello world").unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn call_echo_empty() {
        let mgr = loaded_manager();
        let result = mgr.call_plugin("clankers-test-plugin", "echo", "").unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn call_echo_unicode() {
        let mgr = loaded_manager();
        let result = mgr.call_plugin("clankers-test-plugin", "echo", "🦀 Rust + WASM 🎉").unwrap();
        assert_eq!(result, "🦀 Rust + WASM 🎉");
    }

    #[test]
    fn call_echo_large_input() {
        let mgr = loaded_manager();
        let large = "x".repeat(100_000);
        let result = mgr.call_plugin("clankers-test-plugin", "echo", &large).unwrap();
        assert_eq!(result.len(), 100_000);
    }

    #[test]
    fn call_greet() {
        let mgr = loaded_manager();
        let result = mgr.call_plugin("clankers-test-plugin", "greet", "clankers").unwrap();
        assert_eq!(result, "Hello, clankers! 👋");
    }

    #[test]
    fn call_reverse() {
        let mgr = loaded_manager();
        let result = mgr.call_plugin("clankers-test-plugin", "reverse", "abcdef").unwrap();
        assert_eq!(result, "fedcba");
    }

    #[test]
    fn call_reverse_unicode() {
        let mgr = loaded_manager();
        let result = mgr.call_plugin("clankers-test-plugin", "reverse", "🅰🅱🅲").unwrap();
        assert_eq!(result, "🅲🅱🅰");
    }

    #[test]
    fn call_length() {
        let mgr = loaded_manager();
        let result = mgr.call_plugin("clankers-test-plugin", "length", "hello").unwrap();
        assert_eq!(result, "5");
    }

    #[test]
    fn call_count_words() {
        let mgr = loaded_manager();
        let result = mgr.call_plugin("clankers-test-plugin", "count_words", "the quick brown fox").unwrap();
        assert_eq!(result, "4");
    }

    #[test]
    fn call_count_words_empty() {
        let mgr = loaded_manager();
        let result = mgr.call_plugin("clankers-test-plugin", "count_words", "").unwrap();
        assert_eq!(result, "0");
    }

    #[test]
    fn call_uppercase() {
        let mgr = loaded_manager();
        let result = mgr.call_plugin("clankers-test-plugin", "uppercase", "hello world").unwrap();
        assert_eq!(result, "HELLO WORLD");
    }

    // ── JSON function calls ──────────────────────────────────────────

    #[test]
    fn call_handle_tool_call_echo() {
        let mgr = loaded_manager();
        let input = r#"{"tool":"test_echo","args":{"text":"hello"}}"#;
        let result = mgr.call_plugin("clankers-test-plugin", "handle_tool_call", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["tool"], "test_echo");
        assert_eq!(parsed["result"], "hello");
        assert_eq!(parsed["status"], "ok");
    }

    #[test]
    fn call_handle_tool_call_reverse() {
        let mgr = loaded_manager();
        let input = r#"{"tool":"test_reverse","args":{"text":"abcd"}}"#;
        let result = mgr.call_plugin("clankers-test-plugin", "handle_tool_call", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["tool"], "test_reverse");
        assert_eq!(parsed["result"], "dcba");
        assert_eq!(parsed["status"], "ok");
    }

    #[test]
    fn call_handle_tool_call_unknown_tool() {
        let mgr = loaded_manager();
        let input = r#"{"tool":"unknown_tool","args":{}}"#;
        let result = mgr.call_plugin("clankers-test-plugin", "handle_tool_call", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["status"], "unknown_tool");
    }

    #[test]
    fn call_handle_tool_call_invalid_json_errors() {
        let mgr = loaded_manager();
        let result = mgr.call_plugin("clankers-test-plugin", "handle_tool_call", "not json");
        assert!(result.is_err());
    }

    // ── Event handling ───────────────────────────────────────────────

    #[test]
    fn call_on_event_agent_start() {
        let mgr = loaded_manager();
        let input = r#"{"event":"agent_start","data":{}}"#;
        let result = mgr.call_plugin("clankers-test-plugin", "on_event", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["event"], "agent_start");
        assert_eq!(parsed["handled"], true);
        assert_eq!(parsed["message"], "Test plugin initialized");
    }

    #[test]
    fn call_on_event_agent_end() {
        let mgr = loaded_manager();
        let input = r#"{"event":"agent_end","data":{}}"#;
        let result = mgr.call_plugin("clankers-test-plugin", "on_event", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["handled"], true);
        assert_eq!(parsed["message"], "Test plugin shutting down");
    }

    #[test]
    fn call_on_event_tool_call_with_data() {
        let mgr = loaded_manager();
        let input = r#"{"event":"tool_call","data":{"tool":"grep"}}"#;
        let result = mgr.call_plugin("clankers-test-plugin", "on_event", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["handled"], true);
        assert_eq!(parsed["message"], "Observed tool call: grep");
    }

    #[test]
    fn call_on_event_unknown() {
        let mgr = loaded_manager();
        let input = r#"{"event":"custom_event","data":{}}"#;
        let result = mgr.call_plugin("clankers-test-plugin", "on_event", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["handled"], false);
        assert_eq!(parsed["message"], "Unhandled event: custom_event");
    }

    // ── Plugin metadata (describe) ───────────────────────────────────

    #[test]
    fn call_describe() {
        let mgr = loaded_manager();
        let result = mgr.call_plugin("clankers-test-plugin", "describe", "null").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["name"], "clankers-test-plugin");
        assert_eq!(parsed["version"], "0.1.0");
        let tools = parsed["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "test_echo");
        assert_eq!(tools[1]["name"], "test_reverse");
        let commands = parsed["commands"].as_array().unwrap();
        assert_eq!(commands, &["test"]);
    }

    // ── Error path ───────────────────────────────────────────────────

    #[test]
    fn call_fail_returns_error() {
        let mgr = loaded_manager();
        let result = mgr.call_plugin("clankers-test-plugin", "fail", "");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("intentional test failure"));
    }

    #[test]
    fn call_parse_json_valid() {
        let mgr = loaded_manager();
        let result = mgr.call_plugin("clankers-test-plugin", "parse_json", r#"{"a":1}"#).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["a"], 1);
    }

    #[test]
    fn call_parse_json_invalid() {
        let mgr = loaded_manager();
        let result = mgr.call_plugin("clankers-test-plugin", "parse_json", "not json{");
        assert!(result.is_err());
    }

    #[test]
    fn call_nonexistent_function_errors() {
        let mgr = loaded_manager();
        let result = mgr.call_plugin("clankers-test-plugin", "no_such_fn", "");
        assert!(result.is_err());
    }

    #[test]
    fn call_unloaded_plugin_errors() {
        let mgr = manager_with_test_plugin();
        let result = mgr.call_plugin("clankers-test-plugin", "echo", "test");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not loaded"));
    }

    // ── Reload ───────────────────────────────────────────────────────

    #[test]
    fn reload_plugin() {
        let mut mgr = loaded_manager();
        // Call once before reload
        let r1 = mgr.call_plugin("clankers-test-plugin", "echo", "before").unwrap();
        assert_eq!(r1, "before");

        // Reload
        mgr.reload("clankers-test-plugin").unwrap();

        // Should still work after reload
        let r2 = mgr.call_plugin("clankers-test-plugin", "echo", "after").unwrap();
        assert_eq!(r2, "after");
        assert_eq!(mgr.get("clankers-test-plugin").unwrap().state, PluginState::Active);
    }

    #[test]
    fn reload_all_plugins() {
        let mut mgr = loaded_manager();
        mgr.reload_all();
        // Should still function
        let result = mgr.call_plugin("clankers-test-plugin", "echo", "test").unwrap();
        assert_eq!(result, "test");
    }

    // ── Multiple calls (statefulness check) ──────────────────────────

    #[test]
    fn multiple_sequential_calls() {
        let mgr = loaded_manager();
        for i in 0..100 {
            let input = format!("call-{i}");
            let result = mgr.call_plugin("clankers-test-plugin", "echo", &input).unwrap();
            assert_eq!(result, input);
        }
    }

    // ── Project-dir discovery ────────────────────────────────────────

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
        std::fs::write(&path, r#"{"name":"minimal","version":"0.1.0"}"#).unwrap();
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
        std::fs::write(&path, "not valid json{{{").unwrap();
        assert!(manifest::PluginManifest::load(&path).is_none());
    }

    // ── Sandbox permission tests ─────────────────────────────────────

    #[test]
    fn sandbox_permission_check() {
        use sandbox::Permission;
        use sandbox::has_permission;

        let perms = vec!["fs:read".to_string(), "net".to_string()];
        assert!(has_permission(&perms, Permission::FsRead));
        assert!(has_permission(&perms, Permission::Net));
        assert!(!has_permission(&perms, Permission::FsWrite));
        assert!(!has_permission(&perms, Permission::Exec));
    }

    #[test]
    fn sandbox_all_permission_grants_everything() {
        use sandbox::Permission;
        use sandbox::has_permission;

        let perms = vec!["all".to_string()];
        assert!(has_permission(&perms, Permission::FsRead));
        assert!(has_permission(&perms, Permission::FsWrite));
        assert!(has_permission(&perms, Permission::Net));
        assert!(has_permission(&perms, Permission::Exec));
    }

    #[test]
    fn sandbox_empty_permissions_deny_everything() {
        use sandbox::Permission;
        use sandbox::has_permission;

        let perms: Vec<String> = vec![];
        assert!(!has_permission(&perms, Permission::FsRead));
        assert!(!has_permission(&perms, Permission::Net));
    }

    // ── Registry tests ───────────────────────────────────────────────

    #[test]
    fn registry_register_and_list() {
        let mut reg = registry::PluginRegistry::new();
        reg.register_tool("clankers-test-plugin", "test_echo");
        reg.register_tool("clankers-test-plugin", "test_reverse");
        reg.register_command("clankers-test-plugin", "test");

        let tools = reg.all_tools();
        assert_eq!(tools.len(), 2);
        assert!(tools.contains(&("clankers-test-plugin", "test_echo")));
        assert!(tools.contains(&("clankers-test-plugin", "test_reverse")));

        let commands = reg.all_commands();
        assert_eq!(commands.len(), 1);
        assert!(commands.contains(&("clankers-test-plugin", "test")));
    }

    #[test]
    fn registry_multiple_plugins() {
        let mut reg = registry::PluginRegistry::new();
        reg.register_tool("plugin-a", "tool-1");
        reg.register_tool("plugin-b", "tool-2");

        let tools = reg.all_tools();
        assert_eq!(tools.len(), 2);
    }

    // ── Bridge event parsing tests ───────────────────────────────────

    #[test]
    fn bridge_parse_known_events() {
        use bridge::PluginEvent;

        assert_eq!(PluginEvent::parse("tool_call"), Some(PluginEvent::ToolCall));
        assert_eq!(PluginEvent::parse("tool_result"), Some(PluginEvent::ToolResult));
        assert_eq!(PluginEvent::parse("agent_start"), Some(PluginEvent::AgentStart));
        assert_eq!(PluginEvent::parse("agent_end"), Some(PluginEvent::AgentEnd));
        assert_eq!(PluginEvent::parse("turn_start"), Some(PluginEvent::TurnStart));
        assert_eq!(PluginEvent::parse("turn_end"), Some(PluginEvent::TurnEnd));
        assert_eq!(PluginEvent::parse("message_update"), Some(PluginEvent::MessageUpdate));
        assert_eq!(PluginEvent::parse("user_input"), Some(PluginEvent::UserInput));
    }

    #[test]
    fn bridge_parse_unknown_event() {
        assert_eq!(bridge::PluginEvent::parse("unknown"), None);
        assert_eq!(bridge::PluginEvent::parse(""), None);
    }

    // ── UI widget serialization tests ────────────────────────────────

    #[test]
    fn ui_widget_text_roundtrip() {
        let widget = ui::Widget::Text {
            content: "Hello".to_string(),
            bold: true,
            color: Some("green".to_string()),
        };
        let json = serde_json::to_string(&widget).unwrap();
        let parsed: ui::Widget = serde_json::from_str(&json).unwrap();
        match parsed {
            ui::Widget::Text { content, bold, color } => {
                assert_eq!(content, "Hello");
                assert!(bold);
                assert_eq!(color, Some("green".to_string()));
            }
            _ => panic!("Expected Text widget"),
        }
    }

    #[test]
    fn ui_widget_box_with_children() {
        let widget = ui::Widget::Box {
            children: vec![
                ui::Widget::Text {
                    content: "A".to_string(),
                    bold: false,
                    color: None,
                },
                ui::Widget::Spacer { lines: 2 },
                ui::Widget::Text {
                    content: "B".to_string(),
                    bold: true,
                    color: Some("red".to_string()),
                },
            ],
            direction: ui::Direction::Vertical,
        };
        let json = serde_json::to_string(&widget).unwrap();
        let parsed: ui::Widget = serde_json::from_str(&json).unwrap();
        match parsed {
            ui::Widget::Box { children, .. } => assert_eq!(children.len(), 3),
            _ => panic!("Expected Box widget"),
        }
    }

    #[test]
    fn ui_widget_list() {
        let widget = ui::Widget::List {
            items: vec!["one".to_string(), "two".to_string(), "three".to_string()],
            selected: 1,
        };
        let json = serde_json::to_string(&widget).unwrap();
        assert!(json.contains("\"selected\":1"));
    }

    #[test]
    fn ui_widget_input() {
        let json = r#"{"type":"Input","value":"","placeholder":"Search..."}"#;
        let widget: ui::Widget = serde_json::from_str(json).unwrap();
        match widget {
            ui::Widget::Input { value, placeholder } => {
                assert_eq!(value, "");
                assert_eq!(placeholder, "Search...");
            }
            _ => panic!("Expected Input widget"),
        }
    }

    // ── New widget types ────────────────────────────────────────────

    #[test]
    fn ui_widget_progress_roundtrip() {
        let widget = ui::Widget::Progress {
            label: "Building".to_string(),
            value: 0.75,
            color: Some("green".to_string()),
        };
        let json = serde_json::to_string(&widget).unwrap();
        let parsed: ui::Widget = serde_json::from_str(&json).unwrap();
        match parsed {
            ui::Widget::Progress { label, value, color } => {
                assert_eq!(label, "Building");
                assert!((value - 0.75).abs() < f64::EPSILON);
                assert_eq!(color, Some("green".to_string()));
            }
            _ => panic!("Expected Progress widget"),
        }
    }

    #[test]
    fn ui_widget_table_roundtrip() {
        let widget = ui::Widget::Table {
            headers: vec!["Name".to_string(), "Status".to_string()],
            rows: vec![vec!["plugin-a".to_string(), "active".to_string()], vec![
                "plugin-b".to_string(),
                "error".to_string(),
            ]],
        };
        let json = serde_json::to_string(&widget).unwrap();
        let parsed: ui::Widget = serde_json::from_str(&json).unwrap();
        match parsed {
            ui::Widget::Table { headers, rows } => {
                assert_eq!(headers.len(), 2);
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0][0], "plugin-a");
            }
            _ => panic!("Expected Table widget"),
        }
    }

    // ── PluginUIAction parsing ───────────────────────────────────────

    #[test]
    fn ui_action_set_widget_roundtrip() {
        let action = ui::PluginUIAction::SetWidget {
            plugin: "test".to_string(),
            widget: ui::Widget::Text {
                content: "hello".to_string(),
                bold: true,
                color: None,
            },
        };
        let json = serde_json::to_string(&action).unwrap();
        let parsed: ui::PluginUIAction = serde_json::from_str(&json).unwrap();
        match parsed {
            ui::PluginUIAction::SetWidget { plugin, widget } => {
                assert_eq!(plugin, "test");
                match widget {
                    ui::Widget::Text { content, bold, .. } => {
                        assert_eq!(content, "hello");
                        assert!(bold);
                    }
                    _ => panic!("Expected Text widget"),
                }
            }
            _ => panic!("Expected SetWidget action"),
        }
    }

    #[test]
    fn ui_action_set_status_roundtrip() {
        let action = ui::PluginUIAction::SetStatus {
            plugin: "test".to_string(),
            text: "running".to_string(),
            color: Some("green".to_string()),
        };
        let json = serde_json::to_string(&action).unwrap();
        let parsed: ui::PluginUIAction = serde_json::from_str(&json).unwrap();
        match parsed {
            ui::PluginUIAction::SetStatus { plugin, text, color } => {
                assert_eq!(plugin, "test");
                assert_eq!(text, "running");
                assert_eq!(color, Some("green".to_string()));
            }
            _ => panic!("Expected SetStatus action"),
        }
    }

    #[test]
    fn ui_action_notify_roundtrip() {
        let action = ui::PluginUIAction::Notify {
            plugin: "test".to_string(),
            message: "Build done!".to_string(),
            level: "info".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        let parsed: ui::PluginUIAction = serde_json::from_str(&json).unwrap();
        match parsed {
            ui::PluginUIAction::Notify { plugin, message, level } => {
                assert_eq!(plugin, "test");
                assert_eq!(message, "Build done!");
                assert_eq!(level, "info");
            }
            _ => panic!("Expected Notify action"),
        }
    }

    #[test]
    fn ui_action_clear_widget() {
        let json = r#"{"action":"clear_widget","plugin":"test"}"#;
        let parsed: ui::PluginUIAction = serde_json::from_str(json).unwrap();
        match parsed {
            ui::PluginUIAction::ClearWidget { plugin } => assert_eq!(plugin, "test"),
            _ => panic!("Expected ClearWidget"),
        }
    }

    #[test]
    fn ui_action_clear_status() {
        let json = r#"{"action":"clear_status","plugin":"test"}"#;
        let parsed: ui::PluginUIAction = serde_json::from_str(json).unwrap();
        match parsed {
            ui::PluginUIAction::ClearStatus { plugin } => assert_eq!(plugin, "test"),
            _ => panic!("Expected ClearStatus"),
        }
    }

    // ── PluginUIState tests ──────────────────────────────────────────

    #[test]
    fn plugin_ui_state_set_and_clear_widget() {
        let mut state = ui::PluginUIState::new();
        assert!(!state.has_content());

        state.apply(ui::PluginUIAction::SetWidget {
            plugin: "test".to_string(),
            widget: ui::Widget::Text {
                content: "hello".to_string(),
                bold: false,
                color: None,
            },
        });
        assert!(state.has_content());
        assert!(state.widgets.contains_key("test"));

        state.apply(ui::PluginUIAction::ClearWidget {
            plugin: "test".to_string(),
        });
        assert!(!state.widgets.contains_key("test"));
    }

    #[test]
    fn plugin_ui_state_set_and_clear_status() {
        let mut state = ui::PluginUIState::new();

        state.apply(ui::PluginUIAction::SetStatus {
            plugin: "test".to_string(),
            text: "building".to_string(),
            color: Some("yellow".to_string()),
        });
        assert!(state.has_content());
        let seg = &state.status_segments["test"];
        assert_eq!(seg.text, "building");
        assert_eq!(seg.color, Some("yellow".to_string()));

        state.apply(ui::PluginUIAction::ClearStatus {
            plugin: "test".to_string(),
        });
        assert!(!state.status_segments.contains_key("test"));
    }

    #[test]
    fn plugin_ui_state_notify_and_gc() {
        let mut state = ui::PluginUIState::new();

        state.apply(ui::PluginUIAction::Notify {
            plugin: "test".to_string(),
            message: "hello".to_string(),
            level: "info".to_string(),
        });
        assert_eq!(state.notifications.len(), 1);
        assert_eq!(state.notifications[0].message, "hello");

        // Fresh notifications should survive GC
        state.gc_notifications();
        assert_eq!(state.notifications.len(), 1);
    }

    #[test]
    fn plugin_ui_state_multiple_plugins() {
        let mut state = ui::PluginUIState::new();

        state.apply(ui::PluginUIAction::SetWidget {
            plugin: "plugin-a".to_string(),
            widget: ui::Widget::Text {
                content: "A".to_string(),
                bold: false,
                color: None,
            },
        });
        state.apply(ui::PluginUIAction::SetWidget {
            plugin: "plugin-b".to_string(),
            widget: ui::Widget::Text {
                content: "B".to_string(),
                bold: false,
                color: None,
            },
        });
        state.apply(ui::PluginUIAction::SetStatus {
            plugin: "plugin-a".to_string(),
            text: "ok".to_string(),
            color: None,
        });

        assert_eq!(state.widgets.len(), 2);
        assert_eq!(state.status_segments.len(), 1);
    }

    #[test]
    fn plugin_ui_state_widget_replacement() {
        let mut state = ui::PluginUIState::new();

        state.apply(ui::PluginUIAction::SetWidget {
            plugin: "test".to_string(),
            widget: ui::Widget::Text {
                content: "v1".to_string(),
                bold: false,
                color: None,
            },
        });
        state.apply(ui::PluginUIAction::SetWidget {
            plugin: "test".to_string(),
            widget: ui::Widget::Text {
                content: "v2".to_string(),
                bold: true,
                color: None,
            },
        });

        assert_eq!(state.widgets.len(), 1);
        match &state.widgets["test"] {
            ui::Widget::Text { content, bold, .. } => {
                assert_eq!(content, "v2");
                assert!(*bold);
            }
            _ => panic!("Expected Text widget"),
        }
    }

    // ── Bridge UI action parsing ─────────────────────────────────────

    #[test]
    fn bridge_parse_ui_actions_single() {
        let response = serde_json::json!({
            "handled": true,
            "ui": {
                "action": "set_status",
                "text": "running",
                "color": "green"
            }
        });
        let actions = bridge::parse_ui_actions("my-plugin", &response);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            ui::PluginUIAction::SetStatus { plugin, text, color } => {
                assert_eq!(plugin, "my-plugin");
                assert_eq!(text, "running");
                assert_eq!(color.as_deref(), Some("green"));
            }
            _ => panic!("Expected SetStatus"),
        }
    }

    #[test]
    fn bridge_parse_ui_actions_array() {
        let response = serde_json::json!({
            "handled": true,
            "ui": [
                {"action": "set_status", "text": "ok", "color": "green"},
                {"action": "notify", "message": "done!", "level": "info"},
                {"action": "set_widget", "widget": {"type": "Text", "content": "hi"}}
            ]
        });
        let actions = bridge::parse_ui_actions("test-plugin", &response);
        assert_eq!(actions.len(), 3);
    }

    #[test]
    fn bridge_parse_ui_actions_none() {
        let response = serde_json::json!({"handled": true});
        let actions = bridge::parse_ui_actions("test", &response);
        assert!(actions.is_empty());
    }

    #[test]
    fn bridge_parse_ui_actions_invalid_ignored() {
        let response = serde_json::json!({
            "ui": [
                {"action": "set_status", "text": "ok"},
                {"action": "bogus_action"},
                {"action": "notify", "message": "hi"}
            ]
        });
        let actions = bridge::parse_ui_actions("test", &response);
        // bogus_action should be silently skipped
        assert_eq!(actions.len(), 2);
    }

    #[test]
    fn bridge_parse_ui_actions_injects_plugin_name() {
        let response = serde_json::json!({
            "ui": {"action": "set_status", "text": "test"}
        });
        let actions = bridge::parse_ui_actions("injected-name", &response);
        match &actions[0] {
            ui::PluginUIAction::SetStatus { plugin, .. } => {
                assert_eq!(plugin, "injected-name");
            }
            _ => panic!("Expected SetStatus"),
        }
    }

    #[test]
    fn bridge_parse_ui_actions_preserves_explicit_plugin_name() {
        let response = serde_json::json!({
            "ui": {"action": "set_status", "plugin": "explicit", "text": "test"}
        });
        let actions = bridge::parse_ui_actions("fallback", &response);
        match &actions[0] {
            ui::PluginUIAction::SetStatus { plugin, .. } => {
                assert_eq!(plugin, "explicit");
            }
            _ => panic!("Expected SetStatus"),
        }
    }

    // ── Sandbox UI permission ────────────────────────────────────────

    #[test]
    fn sandbox_ui_permission() {
        use sandbox::Permission;
        use sandbox::has_permission;

        let perms = vec!["ui".to_string()];
        assert!(has_permission(&perms, Permission::Ui));
        assert!(!has_permission(&perms, Permission::FsRead));

        let no_perms: Vec<String> = vec![];
        assert!(!has_permission(&no_perms, Permission::Ui));

        let all_perms = vec!["all".to_string()];
        assert!(has_permission(&all_perms, Permission::Ui));
    }

    // ── Plugin tool integration tests ────────────────────────────────

    #[test]
    fn build_plugin_tools_creates_tools_from_definitions() {
        let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
        let manager = crate::modes::common::init_plugin_manager(&plugins_dir, None, &[]);
        let tools = crate::modes::common::build_plugin_tools(&manager, None);

        // Should have tools from all discovered plugins (test-plugin + self-validate)
        assert!(tools.len() >= 2, "Expected at least 2 plugin tools, got {}", tools.len());

        let names: Vec<String> = tools.iter().map(|t| t.definition().name.clone()).collect();
        assert!(names.contains(&"test_echo".to_string()));
        assert!(names.contains(&"test_reverse".to_string()));

        // Verify descriptions come from tool_definitions
        let echo = tools.iter().find(|t| t.definition().name == "test_echo").unwrap();
        assert!(echo.definition().description.contains("Echo"), "desc: {}", echo.definition().description);
    }

    #[test]
    fn build_all_tools_includes_plugin_tools() {
        let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
        let manager = crate::modes::common::init_plugin_manager(&plugins_dir, None, &[]);
        let tools = crate::modes::common::build_all_tools(None, None, None, Some(&manager), None);

        let names: Vec<String> = tools.iter().map(|t| t.definition().name.clone()).collect();
        // Built-in tools
        assert!(names.contains(&"read".to_string()));
        assert!(names.contains(&"bash".to_string()));
        // Plugin tools
        assert!(names.contains(&"test_echo".to_string()));
        assert!(names.contains(&"test_reverse".to_string()));
    }

    #[test]
    fn build_plugin_tools_empty_when_no_plugins() {
        let dir = tempfile::tempdir().unwrap();
        let manager = crate::modes::common::init_plugin_manager(dir.path(), None, &[]);
        let tools = crate::modes::common::build_plugin_tools(&manager, None);
        assert!(tools.is_empty());
    }

    // ── clankers-hash plugin tests ──────────────────────────────────────

    /// Helper: create a manager and load the hash plugin WASM.
    fn loaded_hash_manager() -> PluginManager {
        let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
        let mut mgr = PluginManager::new(plugins_dir, None);
        mgr.discover();
        mgr.load_wasm("clankers-hash").expect("Failed to load hash plugin WASM");
        mgr
    }

    // ── Hash plugin discovery ───────────────────────────────────────

    #[test]
    fn discover_finds_hash_plugin() {
        let mgr = manager_with_test_plugin();
        assert!(mgr.get("clankers-hash").is_some(), "Hash plugin should be discovered");
    }

    #[test]
    fn discover_reads_hash_manifest_metadata() {
        let mgr = manager_with_test_plugin();
        let info = mgr.get("clankers-hash").unwrap();
        assert_eq!(info.name, "clankers-hash");
        assert_eq!(info.version, "0.1.0");
        assert_eq!(info.state, PluginState::Loaded);
        assert!(info.manifest.description.contains("hash"));
    }

    #[test]
    fn discover_reads_hash_manifest_tools() {
        let mgr = manager_with_test_plugin();
        let info = mgr.get("clankers-hash").unwrap();
        assert_eq!(info.manifest.tools, vec!["hash_text", "encode_text"]);
    }

    #[test]
    fn discover_reads_hash_manifest_events() {
        let mgr = manager_with_test_plugin();
        let info = mgr.get("clankers-hash").unwrap();
        assert_eq!(info.manifest.events, vec!["agent_start", "agent_end"]);
    }

    // ── Hash plugin WASM loading ────────────────────────────────────

    #[test]
    fn load_hash_wasm_transitions_to_active() {
        let mgr = loaded_hash_manager();
        let info = mgr.get("clankers-hash").unwrap();
        assert_eq!(info.state, PluginState::Active);
    }

    // ── Hash plugin function existence ──────────────────────────────

    #[test]
    fn hash_plugin_has_expected_functions() {
        let mgr = loaded_hash_manager();
        assert!(mgr.has_function("clankers-hash", "handle_tool_call"));
        assert!(mgr.has_function("clankers-hash", "on_event"));
        assert!(mgr.has_function("clankers-hash", "describe"));
    }

    // ── hash_text tool calls ────────────────────────────────────────

    #[test]
    fn hash_sha256_hello_world() {
        let mgr = loaded_hash_manager();
        let input = r#"{"tool":"hash_text","args":{"text":"hello world","algorithm":"sha256"}}"#;
        let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["tool"], "hash_text");
        assert_eq!(parsed["result"], "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
    }

    #[test]
    fn hash_sha256_empty() {
        let mgr = loaded_hash_manager();
        let input = r#"{"tool":"hash_text","args":{"text":"","algorithm":"sha256"}}"#;
        let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["result"], "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    #[test]
    fn hash_md5_hello_world() {
        let mgr = loaded_hash_manager();
        let input = r#"{"tool":"hash_text","args":{"text":"hello world","algorithm":"md5"}}"#;
        let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["result"], "5eb63bbbe01eeed093cb22bb8f5acdc3");
    }

    #[test]
    fn hash_md5_empty() {
        let mgr = loaded_hash_manager();
        let input = r#"{"tool":"hash_text","args":{"text":"","algorithm":"md5"}}"#;
        let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["result"], "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn hash_default_algorithm_is_sha256() {
        let mgr = loaded_hash_manager();
        let input = r#"{"tool":"hash_text","args":{"text":"hello world"}}"#;
        let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["status"], "ok");
        // Default should be SHA-256
        assert_eq!(parsed["result"], "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
    }

    // ── encode_text tool calls ──────────────────────────────────────

    #[test]
    fn encode_base64() {
        let mgr = loaded_hash_manager();
        let input = r#"{"tool":"encode_text","args":{"text":"hello world","encoding":"base64","direction":"encode"}}"#;
        let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["result"], "aGVsbG8gd29ybGQ=");
    }

    #[test]
    fn decode_base64() {
        let mgr = loaded_hash_manager();
        let input =
            r#"{"tool":"encode_text","args":{"text":"aGVsbG8gd29ybGQ=","encoding":"base64","direction":"decode"}}"#;
        let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["result"], "hello world");
    }

    #[test]
    fn encode_hex() {
        let mgr = loaded_hash_manager();
        let input = r#"{"tool":"encode_text","args":{"text":"hello","encoding":"hex","direction":"encode"}}"#;
        let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["result"], "68656c6c6f");
    }

    #[test]
    fn decode_hex() {
        let mgr = loaded_hash_manager();
        let input = r#"{"tool":"encode_text","args":{"text":"68656c6c6f","encoding":"hex","direction":"decode"}}"#;
        let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["result"], "hello");
    }

    #[test]
    fn encode_default_is_base64() {
        let mgr = loaded_hash_manager();
        let input = r#"{"tool":"encode_text","args":{"text":"test"}}"#;
        let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["result"], "dGVzdA=="); // base64("test")
    }

    #[test]
    fn encode_default_direction_is_encode() {
        let mgr = loaded_hash_manager();
        let input = r#"{"tool":"encode_text","args":{"text":"test","encoding":"hex"}}"#;
        let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["result"], "74657374"); // hex("test")
    }

    #[test]
    fn decode_invalid_base64_returns_error() {
        let mgr = loaded_hash_manager();
        let input = r#"{"tool":"encode_text","args":{"text":"!!!not-valid-base64!!!","encoding":"base64","direction":"decode"}}"#;
        let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let status = parsed["status"].as_str().unwrap();
        assert!(status.starts_with("error"), "Expected error status, got: {}", status);
    }

    #[test]
    fn decode_invalid_hex_returns_error() {
        let mgr = loaded_hash_manager();
        let input = r#"{"tool":"encode_text","args":{"text":"xyz","encoding":"hex","direction":"decode"}}"#;
        let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let status = parsed["status"].as_str().unwrap();
        assert!(status.starts_with("error"), "Expected error status, got: {}", status);
    }

    // ── Hash plugin event handling ──────────────────────────────────

    #[test]
    fn hash_on_event_agent_start() {
        let mgr = loaded_hash_manager();
        let input = r#"{"event":"agent_start","data":{}}"#;
        let result = mgr.call_plugin("clankers-hash", "on_event", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["handled"], true);
        assert_eq!(parsed["event"], "agent_start");
    }

    #[test]
    fn hash_on_event_agent_end() {
        let mgr = loaded_hash_manager();
        let input = r#"{"event":"agent_end","data":{}}"#;
        let result = mgr.call_plugin("clankers-hash", "on_event", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["handled"], true);
    }

    #[test]
    fn hash_on_event_unknown() {
        let mgr = loaded_hash_manager();
        let input = r#"{"event":"custom_event","data":{}}"#;
        let result = mgr.call_plugin("clankers-hash", "on_event", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["handled"], false);
    }

    // ── Hash plugin unknown tool ────────────────────────────────────

    #[test]
    fn hash_unknown_tool_returns_unknown_status() {
        let mgr = loaded_hash_manager();
        let input = r#"{"tool":"nonexistent","args":{}}"#;
        let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["status"], "unknown_tool");
    }

    // ── Hash plugin describe ────────────────────────────────────────

    #[test]
    fn hash_describe_metadata() {
        let mgr = loaded_hash_manager();
        let result = mgr.call_plugin("clankers-hash", "describe", "null").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["name"], "clankers-hash");
        assert_eq!(parsed["version"], "0.1.0");
        let tools = parsed["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "hash_text");
        assert_eq!(tools[1]["name"], "encode_text");
    }

    // ── Hash plugin tool integration ────────────────────────────────

    #[test]
    fn build_plugin_tools_includes_hash_tools() {
        let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
        let manager = crate::modes::common::init_plugin_manager(&plugins_dir, None, &[]);
        let tools = crate::modes::common::build_plugin_tools(&manager, None);

        let names: Vec<String> = tools.iter().map(|t| t.definition().name.clone()).collect();
        assert!(names.contains(&"hash_text".to_string()), "Should have hash_text tool");
        assert!(names.contains(&"encode_text".to_string()), "Should have encode_text tool");
    }
}
