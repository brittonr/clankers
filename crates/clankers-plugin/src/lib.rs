//! Plugin system (Extism WASM)
//!
//! Core plugin manager, manifest loading, WASM execution, sandboxing,
//! host functions, and UI action protocol.

pub mod bridge;
pub mod hooks;
pub mod host;
pub mod manifest;
pub mod registry;
pub mod sandbox;
pub mod ui;

#[cfg(test)]
#[path = "sandbox_tests.rs"]
mod sandbox_tests;

#[cfg(test)]
#[path = "host_tests.rs"]
mod host_tests;

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod bridge_tests;

use std::collections::HashMap;
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

        let has_net = sandbox::has_permission(&info.manifest.permissions, sandbox::Permission::Net);
        let mut manifest = extism::Manifest::new([extism::Wasm::file(&wasm_path)]);

        // HTTP sandboxing: only plugins with "net" permission get allowed_hosts
        if has_net {
            let hosts = info.manifest.allowed_hosts.clone().unwrap_or_else(|| vec!["*".to_string()]);
            manifest = manifest.with_allowed_hosts(hosts.into_iter());
        }

        // Config injection: resolve env var names from manifest → Extism config
        for (config_key, env_var) in &info.manifest.config_env {
            if let Ok(val) = std::env::var(env_var) {
                manifest = manifest.with_config_key(config_key, val);
            }
        }

        // Always inject current UTC time so plugins can do time-aware work.
        {
            let now = chrono::Utc::now();
            manifest = manifest.with_config_key("current_time", now.format("%Y%m%dT%H%M%SZ").to_string());
            manifest = manifest.with_config_key("current_time_unix", now.timestamp().to_string());
        }

        // Timeout for network plugins
        if has_net {
            manifest = manifest.with_timeout(std::time::Duration::from_secs(30));
        }

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

    /// Call a function on a loaded plugin.
    ///
    /// Recovers from poisoned mutexes (e.g. if a previous call panicked)
    /// and isolates plugin errors so one bad plugin can't take down others.
    pub fn call_plugin(&self, name: &str, function: &str, input: &str) -> Result<String, String> {
        let instance = self.instances.get(name).ok_or_else(|| format!("Plugin '{}' not loaded", name))?;

        let mut plugin = instance.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("Plugin '{}' mutex was poisoned, recovering", name);
            poisoned.into_inner()
        });

        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| plugin.call::<&str, String>(function, input))) {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(format!("Plugin call error: {}", e)),
            Err(_) => {
                tracing::error!("Plugin '{}' panicked during {}()", name, function);
                Err(format!("Plugin '{}' panicked during {}()", name, function))
            }
        }
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

    /// Disable a plugin (unload WASM, set state to Disabled).
    pub fn disable(&mut self, name: &str) -> Result<(), String> {
        let info = self.plugins.get_mut(name).ok_or_else(|| format!("Plugin '{}' not found", name))?;
        self.instances.remove(name);
        info.state = PluginState::Disabled;
        Ok(())
    }

    /// Enable a previously disabled plugin (reload its WASM).
    pub fn enable(&mut self, name: &str) -> Result<(), String> {
        let info = self.plugins.get(name).ok_or_else(|| format!("Plugin '{}' not found", name))?;
        if info.state != PluginState::Disabled {
            return Err(format!("Plugin '{}' is not disabled (state: {:?})", name, info.state));
        }
        self.load_wasm(name)
    }

    /// Reload a plugin (unload + re-load WASM).
    pub fn reload(&mut self, name: &str) -> Result<(), String> {
        self.instances.remove(name);
        self.load_wasm(name)
    }

    /// Reload all non-disabled plugins.
    pub fn reload_all(&mut self) {
        let names: Vec<String> = self
            .plugins
            .iter()
            .filter(|(_, info)| info.state != PluginState::Disabled)
            .map(|(name, _)| name.clone())
            .collect();
        for name in names {
            self.reload(&name).ok();
        }
    }

    /// Get the names of all disabled plugins.
    pub fn disabled_plugins(&self) -> Vec<String> {
        self.plugins
            .iter()
            .filter(|(_, info)| info.state == PluginState::Disabled)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Iterate active/loaded plugins (for external trait impls and contribution collection).
    pub fn active_plugin_infos(&self) -> impl Iterator<Item = &PluginInfo> {
        self.plugins.values().filter(|p| matches!(p.state, PluginState::Loaded | PluginState::Active))
    }

    /// Inject a pre-built WASM plugin instance (for testing).
    pub fn inject_instance(&mut self, name: String, plugin: extism::Plugin) {
        self.instances.insert(name, Mutex::new(plugin));
    }

    /// Get mutable access to a plugin's info (for testing state overrides).
    pub fn get_mut(&mut self, name: &str) -> Option<&mut PluginInfo> {
        self.plugins.get_mut(name)
    }

    /// Disable plugins by name (used to restore persisted disabled state).
    pub fn apply_disabled_set(&mut self, disabled: &[String]) {
        for name in disabled {
            if self.plugins.contains_key(name) {
                self.disable(name).ok();
            }
        }
    }
}

/// Load plugins from a directory (shared helper for discover).
pub fn load_plugins_from_dir(dir: &std::path::Path, plugins: &mut HashMap<String, PluginInfo>) {
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
