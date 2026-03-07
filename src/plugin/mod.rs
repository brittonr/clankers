//! Plugin system (Extism WASM)

pub mod bridge;
mod contributions;
pub mod host;
pub mod manifest;
pub mod registry;
pub mod sandbox;
pub mod ui;

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
        contributions::load_plugins_from_dir(&self.global_dir, &mut self.plugins);
        if let Some(ref dir) = self.project_dir {
            contributions::load_plugins_from_dir(dir, &mut self.plugins);
        }
        for dir in &self.extra_dirs {
            contributions::load_plugins_from_dir(dir, &mut self.plugins);
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
            let hosts = info.manifest.allowed_hosts
                .clone()
                .unwrap_or_else(|| vec!["*".to_string()]);
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
            manifest = manifest.with_config_key(
                "current_time",
                now.format("%Y%m%dT%H%M%SZ").to_string(),
            );
            manifest = manifest.with_config_key(
                "current_time_unix",
                now.timestamp().to_string(),
            );
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

#[cfg(test)]
mod tests;
