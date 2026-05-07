//! Explicit desktop/default service adapters for the embeddable runtime.
//!
//! These adapters make the normal Clankers path layout an explicit host-owned choice instead of
//! letting `clankers-runtime` discover `~/.clankers` or project `.clankers` paths implicitly.

use std::path::PathBuf;
use std::sync::Arc;

use clankers_runtime::AuthService;
use clankers_runtime::CacheStore;
use clankers_runtime::CheckpointStore;
use clankers_runtime::CredentialPoolPolicyService;
use clankers_runtime::CredentialPoolRequest;
use clankers_runtime::ExtensionAuthStoreService;
use clankers_runtime::ExtensionReceipt;
use clankers_runtime::ExtensionRuntimeKind;
use clankers_runtime::ExtensionRuntimeRequest;
use clankers_runtime::ExtensionRuntimeService;
use clankers_runtime::ExtensionServices;
use clankers_runtime::ExtensionToolDescriptor;
use clankers_runtime::PluginStore;
use clankers_runtime::ProjectContextService;
use clankers_runtime::ProviderExecutionRequest;
use clankers_runtime::ProviderRouterService;
use clankers_runtime::RuntimeError;
use clankers_runtime::RuntimeServices;
use clankers_runtime::SessionId;
use clankers_runtime::SessionRecord;
use clankers_runtime::SessionStore;
use clankers_runtime::SettingsService;
use clankers_runtime::SideEffectLevel;
use clankers_runtime::SkillStore;

/// Explicit adapter bundle for the normal desktop Clankers path layout.
pub struct DesktopRuntimeServiceAdapters;

impl DesktopRuntimeServiceAdapters {
    #[must_use]
    pub fn from_paths(
        paths: &crate::config::ClankersPaths,
        project_paths: &crate::config::ProjectPaths,
    ) -> RuntimeServices {
        Self::from_paths_with_optional_plugin_manager(paths, project_paths, None)
    }

    #[must_use]
    pub fn from_paths_with_plugin_manager(
        paths: &crate::config::ClankersPaths,
        project_paths: &crate::config::ProjectPaths,
        plugin_manager: Arc<std::sync::Mutex<crate::plugin::PluginManager>>,
    ) -> RuntimeServices {
        Self::from_paths_with_optional_plugin_manager(paths, project_paths, Some(plugin_manager))
    }

    fn from_paths_with_optional_plugin_manager(
        paths: &crate::config::ClankersPaths,
        project_paths: &crate::config::ProjectPaths,
        plugin_manager: Option<Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    ) -> RuntimeServices {
        let settings = Arc::new(DesktopSettingsService {
            global_settings: paths.global_settings.clone(),
            project_settings: project_paths.settings.clone(),
        });
        let auth = Arc::new(DesktopAuthService {
            auth_file: paths.global_auth.clone(),
        });
        let sessions = Arc::new(DesktopSessionStore {
            sessions_dir: paths.global_sessions_dir.clone(),
            memory: clankers_runtime::InMemorySessionStore::default(),
        });
        let cache = Arc::new(DesktopCacheStore {
            cache_dir: paths.global_config_dir.join("cache"),
        });
        let project_context = Arc::new(DesktopProjectContextService {
            root: project_paths.root.clone(),
            config_dir: project_paths.config_dir.clone(),
        });
        let skills = Arc::new(DesktopSkillStore {
            global_skills_dir: paths.global_skills_dir.clone(),
            project_skills_dir: project_paths.skills_dir.clone(),
        });
        let plugins = Arc::new(DesktopPluginStore {
            global_plugins_dir: paths.global_plugins_dir.clone(),
            project_plugins_dir: project_paths.plugins_dir.clone(),
        });
        let checkpoints = Arc::new(DesktopCheckpointStore {
            checkpoints_dir: project_paths.config_dir.join("checkpoints"),
        });
        let extensions = ExtensionServices {
            provider_router: Arc::new(DesktopProviderRouterService),
            auth_store: Arc::new(DesktopExtensionAuthStoreService),
            credential_pool: Arc::new(DesktopCredentialPoolPolicyService),
            runtime: Arc::new(DesktopExtensionRuntimeService { plugin_manager }),
        };
        RuntimeServices {
            settings,
            auth,
            sessions,
            cache,
            project_context,
            skills,
            plugins,
            checkpoints,
            extensions,
        }
    }
}

struct DesktopSettingsService {
    global_settings: PathBuf,
    project_settings: PathBuf,
}
struct DesktopAuthService {
    auth_file: PathBuf,
}
struct DesktopSessionStore {
    sessions_dir: PathBuf,
    memory: clankers_runtime::InMemorySessionStore,
}
struct DesktopCacheStore {
    cache_dir: PathBuf,
}
struct DesktopProjectContextService {
    root: PathBuf,
    config_dir: PathBuf,
}
struct DesktopSkillStore {
    global_skills_dir: PathBuf,
    project_skills_dir: PathBuf,
}
struct DesktopPluginStore {
    global_plugins_dir: PathBuf,
    project_plugins_dir: PathBuf,
}
struct DesktopCheckpointStore {
    checkpoints_dir: PathBuf,
}

impl SettingsService for DesktopSettingsService {
    fn capability(&self) -> &'static str {
        let _ = (&self.global_settings, &self.project_settings);
        "desktop_settings"
    }
}
impl AuthService for DesktopAuthService {
    fn capability(&self) -> &'static str {
        let _ = &self.auth_file;
        "desktop_auth"
    }
}
impl SessionStore for DesktopSessionStore {
    fn capability(&self) -> &'static str {
        let _ = &self.sessions_dir;
        "desktop_sessions"
    }

    fn save(&self, record: SessionRecord) -> Result<(), RuntimeError> {
        self.memory.save(record)
    }

    fn load(&self, session_id: &SessionId) -> Result<Option<SessionRecord>, RuntimeError> {
        self.memory.load(session_id)
    }
}
impl CacheStore for DesktopCacheStore {
    fn capability(&self) -> &'static str {
        let _ = &self.cache_dir;
        "desktop_cache"
    }
}
impl ProjectContextService for DesktopProjectContextService {
    fn capability(&self) -> &'static str {
        let _ = (&self.root, &self.config_dir);
        "desktop_project_context"
    }
}
impl SkillStore for DesktopSkillStore {
    fn capability(&self) -> &'static str {
        let _ = (&self.global_skills_dir, &self.project_skills_dir);
        "desktop_skills"
    }
}
impl PluginStore for DesktopPluginStore {
    fn capability(&self) -> &'static str {
        let _ = (&self.global_plugins_dir, &self.project_plugins_dir);
        "desktop_plugins"
    }
}
impl CheckpointStore for DesktopCheckpointStore {
    fn capability(&self) -> &'static str {
        let _ = &self.checkpoints_dir;
        "desktop_checkpoints"
    }
}

struct DesktopProviderRouterService;
struct DesktopExtensionAuthStoreService;
struct DesktopCredentialPoolPolicyService;
struct DesktopExtensionRuntimeService {
    plugin_manager: Option<Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
}

impl ProviderRouterService for DesktopProviderRouterService {
    fn capability(&self) -> &'static str {
        "desktop_provider_router"
    }

    fn execute(&self, request: ProviderExecutionRequest) -> Result<ExtensionReceipt, RuntimeError> {
        Ok(
            ExtensionReceipt::new("desktop_provider_router", "execute", clankers_runtime::ExtensionStatus::Succeeded)
                .with_metadata("provider", request.provider)
                .with_metadata("route_source", request.route_source),
        )
    }
}

impl ExtensionAuthStoreService for DesktopExtensionAuthStoreService {
    fn capability(&self) -> &'static str {
        "desktop_extension_auth_store"
    }

    fn access(&self, request: clankers_runtime::AuthStoreAccessRequest) -> Result<ExtensionReceipt, RuntimeError> {
        Ok(ExtensionReceipt::new(
            "desktop_extension_auth_store",
            format!("{:?}", request.operation),
            clankers_runtime::ExtensionStatus::Succeeded,
        )
        .with_metadata("provider", request.provider))
    }
}

impl CredentialPoolPolicyService for DesktopCredentialPoolPolicyService {
    fn capability(&self) -> &'static str {
        "desktop_credential_pool"
    }

    fn select(&self, request: CredentialPoolRequest) -> Result<ExtensionReceipt, RuntimeError> {
        Ok(
            ExtensionReceipt::new("desktop_credential_pool", "select", clankers_runtime::ExtensionStatus::Succeeded)
                .with_metadata("provider", request.provider)
                .with_metadata("strategy", request.strategy),
        )
    }
}

impl ExtensionRuntimeService for DesktopExtensionRuntimeService {
    fn capability(&self) -> &'static str {
        "desktop_extension_runtime"
    }

    fn publishable_tools(&self, kind: ExtensionRuntimeKind) -> Result<Vec<ExtensionToolDescriptor>, RuntimeError> {
        if kind != ExtensionRuntimeKind::Plugin {
            return Ok(Vec::new());
        }
        let Some(plugin_manager) = &self.plugin_manager else {
            return Ok(Vec::new());
        };
        let host = crate::plugin::PluginHostFacade::new(Arc::clone(plugin_manager));
        let mut descriptors = Vec::new();
        for plugin in host.active_plugins() {
            if !plugin.manifest.kind.uses_wasm_runtime() {
                continue;
            }
            if plugin.manifest.tool_definitions.is_empty() {
                for tool_name in &plugin.manifest.tools {
                    descriptors.push(
                        ExtensionToolDescriptor::new(
                            ExtensionRuntimeKind::Plugin,
                            tool_name.clone(),
                            Some(tool_name.clone()),
                            SideEffectLevel::ExternalIo,
                        )
                        .with_metadata("plugin", plugin.name.clone())
                        .with_metadata("runtime_entrypoint", "handle_tool_call"),
                    );
                }
            } else {
                for tool in &plugin.manifest.tool_definitions {
                    descriptors.push(
                        ExtensionToolDescriptor::new(
                            ExtensionRuntimeKind::Plugin,
                            tool.name.clone(),
                            Some(tool.name.clone()),
                            SideEffectLevel::ExternalIo,
                        )
                        .with_metadata("plugin", plugin.name.clone())
                        .with_metadata("runtime_entrypoint", tool.handler.clone()),
                    );
                }
            }
        }
        Ok(descriptors)
    }

    fn execute(&self, request: ExtensionRuntimeRequest) -> Result<ExtensionReceipt, RuntimeError> {
        if request.kind != ExtensionRuntimeKind::Plugin {
            return Err(RuntimeError::ExtensionUnavailable("unsupported desktop extension runtime kind".to_string()));
        }
        let Some(plugin_manager) = &self.plugin_manager else {
            return Err(RuntimeError::ExtensionUnavailable("desktop plugin runtime not injected".to_string()));
        };
        let plugin_name = request
            .extension_name
            .clone()
            .or_else(|| (!request.action.trim().is_empty()).then_some(request.action.clone()))
            .ok_or_else(|| RuntimeError::InvalidTool("plugin runtime request missing extension name".to_string()))?;
        let visible_tool = request
            .visible_tool_name
            .clone()
            .or_else(|| request.original_tool_name.clone())
            .unwrap_or_else(|| "plugin_tool".to_string());
        let handler = request.runtime_entrypoint.clone().unwrap_or_else(|| "handle_tool_call".to_string());
        let envelope = serde_json::json!({
            "tool": visible_tool,
            "args": request.arguments,
        });
        let input = serde_json::to_string(&envelope)
            .map_err(|error| RuntimeError::InvalidTool(format!("plugin runtime request encoding failed: {error}")))?;
        let output = {
            let manager = plugin_manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            manager.call_plugin(&plugin_name, &handler, &input)
        };
        match output {
            Ok(output) => Ok(ExtensionReceipt::new(
                "desktop_plugin_runtime",
                "execute",
                clankers_runtime::ExtensionStatus::Succeeded,
            )
            .with_metadata("plugin", plugin_name)
            .with_metadata("visible_tool", visible_tool)
            .with_metadata("runtime_entrypoint", handler)
            .with_metadata("output_bytes", output.len().to_string())),
            Err(_error) => Ok(ExtensionReceipt::new(
                "desktop_plugin_runtime",
                "execute",
                clankers_runtime::ExtensionStatus::Failed,
            )
            .with_error_class(clankers_runtime::ErrorClass::Extension)
            .with_metadata("plugin", plugin_name)
            .with_metadata("visible_tool", visible_tool)
            .with_metadata("runtime_entrypoint", handler)
            .with_metadata("error", "plugin_call_failed")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;

    use clankers_runtime::ExtensionRuntimeKind;
    use clankers_runtime::ExtensionRuntimeRequest;
    use clankers_runtime::ExtensionStatus;
    use clankers_runtime::RuntimeError;

    use super::DesktopRuntimeServiceAdapters;

    #[test]
    fn desktop_runtime_services_publish_explicit_capabilities() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());

        let services = DesktopRuntimeServiceAdapters::from_paths(&paths, &project_paths);
        let metadata = services.capability_metadata();

        assert_eq!(metadata.fields.get("settings").unwrap(), "desktop_settings");
        assert_eq!(metadata.fields.get("auth").unwrap(), "desktop_auth");
        assert_eq!(metadata.fields.get("sessions").unwrap(), "desktop_sessions");
        assert_eq!(metadata.fields.get("plugins").unwrap(), "desktop_plugins");
        assert_eq!(metadata.fields.get("project_context").unwrap(), "desktop_project_context");
        assert_eq!(metadata.fields.get("provider_router").unwrap(), "desktop_provider_router");
        assert_eq!(metadata.fields.get("extension_auth_store").unwrap(), "desktop_extension_auth_store");
        assert_eq!(metadata.fields.get("credential_pool").unwrap(), "desktop_credential_pool");
        assert_eq!(metadata.fields.get("extension_runtime").unwrap(), "desktop_extension_runtime");
    }

    #[test]
    fn desktop_runtime_extension_executes_injected_wasm_plugin_with_safe_receipt() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());
        let plugins_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
        let mut plugin_manager = crate::plugin::PluginManager::new(plugins_dir, None);
        plugin_manager.discover();
        plugin_manager.load_wasm("clankers-test-plugin").expect("test plugin loads");
        let plugin_manager = Arc::new(Mutex::new(plugin_manager));

        let services =
            DesktopRuntimeServiceAdapters::from_paths_with_plugin_manager(&paths, &project_paths, plugin_manager);
        let descriptors = services
            .extensions
            .runtime
            .publishable_tools(ExtensionRuntimeKind::Plugin)
            .expect("plugin descriptors");
        assert!(descriptors.iter().any(|descriptor| descriptor.visible_tool_name == "test_echo"));

        let receipt = services
            .extensions
            .runtime
            .execute(ExtensionRuntimeRequest {
                kind: ExtensionRuntimeKind::Plugin,
                action: "call".to_string(),
                extension_name: Some("clankers-test-plugin".to_string()),
                visible_tool_name: Some("test_echo".to_string()),
                original_tool_name: Some("test_echo".to_string()),
                runtime_entrypoint: Some("handle_tool_call".to_string()),
                arguments: serde_json::json!({"text": "hello via runtime seam"}),
            })
            .expect("plugin receipt");

        assert_eq!(receipt.status, ExtensionStatus::Succeeded);
        assert_eq!(receipt.source, "desktop_plugin_runtime");
        assert_eq!(receipt.metadata.fields.get("plugin").unwrap(), "clankers-test-plugin");
        assert_eq!(receipt.metadata.fields.get("visible_tool").unwrap(), "test_echo");
        assert_eq!(receipt.metadata.fields.get("runtime_entrypoint").unwrap(), "handle_tool_call");
        assert!(receipt.metadata.fields.contains_key("output_bytes"));
        assert!(!receipt.metadata.fields.values().any(|value| value.contains("hello via runtime seam")));
        assert!(!receipt.contains_secret_markers());
    }

    #[test]
    fn desktop_runtime_extension_fails_closed_without_injected_plugin_manager() {
        let paths = crate::config::ClankersPaths::resolve();
        let temp = tempfile::tempdir().expect("temp project root");
        let project_paths = crate::config::ProjectPaths::resolve(temp.path());
        let services = DesktopRuntimeServiceAdapters::from_paths(&paths, &project_paths);

        let error = services
            .extensions
            .runtime
            .execute(ExtensionRuntimeRequest {
                kind: ExtensionRuntimeKind::Plugin,
                action: "call".to_string(),
                extension_name: Some("clankers-test-plugin".to_string()),
                visible_tool_name: Some("test_echo".to_string()),
                original_tool_name: Some("test_echo".to_string()),
                runtime_entrypoint: Some("handle_tool_call".to_string()),
                arguments: serde_json::json!({"text": "not executed"}),
            })
            .unwrap_err();

        assert_eq!(error, RuntimeError::ExtensionUnavailable("desktop plugin runtime not injected".to_string()));
    }
}
