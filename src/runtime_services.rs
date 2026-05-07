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
use clankers_runtime::SkillStore;

/// Explicit adapter bundle for the normal desktop Clankers path layout.
pub struct DesktopRuntimeServiceAdapters;

impl DesktopRuntimeServiceAdapters {
    #[must_use]
    pub fn from_paths(
        paths: &crate::config::ClankersPaths,
        project_paths: &crate::config::ProjectPaths,
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
            runtime: Arc::new(DesktopExtensionRuntimeService),
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
struct DesktopExtensionRuntimeService;

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
        let _ = kind;
        Ok(Vec::new())
    }

    fn execute(&self, request: ExtensionRuntimeRequest) -> Result<ExtensionReceipt, RuntimeError> {
        Ok(ExtensionReceipt::new(
            "desktop_extension_runtime",
            request.action,
            clankers_runtime::ExtensionStatus::Succeeded,
        ))
    }
}

#[cfg(test)]
mod tests {
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
}
