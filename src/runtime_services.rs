//! Explicit desktop/default service adapters for the embeddable runtime.
//!
//! These adapters make the normal Clankers path layout an explicit host-owned choice instead of
//! letting `clankers-runtime` discover `~/.clankers` or project `.clankers` paths implicitly.

use std::path::PathBuf;
use std::sync::Arc;

use clankers_runtime::AuthService;
use clankers_runtime::CacheStore;
use clankers_runtime::CheckpointStore;
use clankers_runtime::PluginStore;
use clankers_runtime::ProjectContextService;
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
        RuntimeServices {
            settings,
            auth,
            sessions,
            cache,
            project_context,
            skills,
            plugins,
            checkpoints,
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
    }
}
