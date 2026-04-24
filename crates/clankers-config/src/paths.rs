//! XDG / ~/.clankers/agent/ path resolution
//!
//! Also supports reading from ~/.pi/agent/ as a fallback for users
//! migrating from pi, so existing auth and settings are picked up.

use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

/// All resolved paths for clankers configuration and data
#[derive(Debug, Clone)]
pub struct ClankersPaths {
    /// Global config directory: ~/.clankers/agent/
    pub global_config_dir: PathBuf,
    /// Global settings file: ~/.clankers/agent/settings.json
    pub global_settings: PathBuf,
    /// Global Nickel settings file: ~/.clankers/agent/settings.ncl
    pub global_settings_ncl: PathBuf,
    /// Global auth file: ~/.config/clanker-router/auth.json (shared with clanker-router)
    pub global_auth: PathBuf,
    /// Global agents directory: ~/.clankers/agent/agents/
    pub global_agents_dir: PathBuf,
    /// Global skills directory: ~/.clankers/agent/skills/
    pub global_skills_dir: PathBuf,
    /// Global prompts directory: ~/.clankers/agent/prompts/
    pub global_prompts_dir: PathBuf,
    /// Global plugins directory: ~/.clankers/agent/plugins/
    pub global_plugins_dir: PathBuf,
    /// Global sessions directory: ~/.clankers/agent/sessions/
    pub global_sessions_dir: PathBuf,
    /// Global themes directory: ~/.clankers/agent/themes/
    pub global_themes_dir: PathBuf,

    /// Fallback pi config directory: ~/.pi/agent/
    /// Used for reading auth/settings when ~/.clankers/agent/ versions don't exist
    pub pi_config_dir: Option<PathBuf>,
    /// Fallback pi settings: ~/.pi/agent/settings.json
    pub pi_settings: Option<PathBuf>,
    /// Fallback pi auth: ~/.pi/agent/auth.json
    pub pi_auth: Option<PathBuf>,
}

/// Project-local paths (relative to project root)
#[derive(Debug, Clone)]
pub struct ProjectPaths {
    /// Project root (where .clankers/ lives or cwd)
    pub root: PathBuf,
    /// Project config directory: .clankers/
    pub config_dir: PathBuf,
    /// Project settings: .clankers/settings.json
    pub settings: PathBuf,
    /// Project Nickel settings: .clankers/settings.ncl
    pub settings_ncl: PathBuf,
    /// Project agents: .clankers/agents/
    pub agents_dir: PathBuf,
    /// Project skills: .clankers/skills/
    pub skills_dir: PathBuf,
    /// Project plugins: .clankers/plugins/
    pub plugins_dir: PathBuf,
    /// Project root-level plugins: plugins/
    pub plugins_root_dir: PathBuf,
    /// Project prompts: .clankers/prompts/
    pub prompts_dir: PathBuf,
    /// Project context files: .clankers/context.md, .clankers/context/
    pub context_file: PathBuf,
    pub context_dir: PathBuf,
    /// OpenSpec directory: openspec/
    pub spec_dir: PathBuf,
}

/// Process-wide cached paths (resolved once on first access).
static CACHED_PATHS: OnceLock<ClankersPaths> = OnceLock::new();

impl ClankersPaths {
    /// Returns a reference to the globally-cached paths.
    ///
    /// Resolves on first call, returns the cached value on subsequent calls.
    /// Prefer this over [`resolve()`](Self::resolve) to avoid redundant
    /// XDG/home-directory lookups.
    pub fn get() -> &'static ClankersPaths {
        CACHED_PATHS.get_or_init(Self::resolve)
    }

    /// Resolve global paths using home directory.
    /// Also detects ~/.pi/agent/ as a fallback source for auth and settings.
    pub fn resolve() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
        let base = home.join(".clankers").join("agent");

        // Check for ~/.pi/agent/ fallback
        let pi_base = home.join(".pi").join("agent");
        let (pi_config_dir, pi_settings, pi_auth) = if pi_base.is_dir() {
            let settings = pi_base.join("settings.json");
            let auth = pi_base.join("auth.json");
            (
                Some(pi_base),
                if settings.exists() { Some(settings) } else { None },
                if auth.exists() { Some(auth) } else { None },
            )
        } else {
            (None, None, None)
        };

        // Auth is shared with clanker-router at the XDG config location.
        // Service deployments can override this with either a direct auth file
        // or a seed/runtime pair. For the seed/runtime pair, materialize the
        // merged effective auth store into the runtime path at process start so
        // existing callers can keep using a single path.
        let auth_path = if let Ok(path) = std::env::var("CLANKERS_AUTH_FILE") {
            PathBuf::from(path)
        } else {
            let seed = std::env::var("CLANKERS_AUTH_SEED_FILE").ok().filter(|value| !value.is_empty());
            let runtime = std::env::var("CLANKERS_AUTH_RUNTIME_FILE").ok().filter(|value| !value.is_empty());
            if let (Some(seed), Some(runtime)) = (seed, runtime) {
                let runtime_path = PathBuf::from(runtime);
                let auth_paths =
                    clanker_router::auth::AuthStorePaths::layered(PathBuf::from(seed), runtime_path.clone());
                if let Some(parent) = runtime_path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                let effective = auth_paths.load_effective().into_store();
                effective.save(&runtime_path).ok();
                runtime_path
            } else {
                dirs::config_dir().unwrap_or_else(|| home.join(".config")).join("clanker-router").join("auth.json")
            }
        };

        Self {
            global_settings: base.join("settings.json"),
            global_settings_ncl: base.join("settings.ncl"),
            global_auth: auth_path,
            global_agents_dir: base.join("agents"),
            global_skills_dir: base.join("skills"),
            global_prompts_dir: base.join("prompts"),
            global_plugins_dir: base.join("plugins"),
            global_sessions_dir: base.join("sessions"),
            global_themes_dir: base.join("themes"),
            global_config_dir: base,
            pi_config_dir,
            pi_settings,
            pi_auth,
        }
    }
}

impl ProjectPaths {
    /// Resolve project paths from a given working directory.
    /// Walks up parent directories looking for .clankers/ directory.
    pub fn resolve(cwd: &Path) -> Self {
        let root = find_project_root(cwd).unwrap_or_else(|| cwd.to_path_buf());
        let config_dir = root.join(".clankers");
        Self {
            settings: config_dir.join("settings.json"),
            settings_ncl: config_dir.join("settings.ncl"),
            agents_dir: config_dir.join("agents"),
            skills_dir: config_dir.join("skills"),
            plugins_dir: config_dir.join("plugins"),
            plugins_root_dir: root.join("plugins"),
            prompts_dir: config_dir.join("prompts"),
            context_file: config_dir.join("context.md"),
            context_dir: config_dir.join("context"),
            spec_dir: root.join("openspec"),
            config_dir,
            root,
        }
    }
}

/// Walk up from cwd looking for a .clankers/ directory
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(unbounded_loop, reason = "traversal loop; bounded by filesystem depth")
)]
fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".clankers").is_dir() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}
