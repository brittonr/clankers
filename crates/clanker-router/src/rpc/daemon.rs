//! Daemon lifecycle management (pid file, auto-start)

use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

/// Daemon metadata written to `daemon.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonInfo {
    /// Iroh node ID (hex-encoded public key)
    pub node_id: String,
    /// Process ID of the daemon
    pub pid: u32,
    /// Direct socket addresses for the iroh endpoint (e.g. "127.0.0.1:12345")
    #[serde(default)]
    pub addrs: Vec<String>,
}

/// Default path for daemon.json.
pub fn daemon_info_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("clanker-router").join("daemon.json")
}

impl DaemonInfo {
    /// Write daemon info to disk.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Load daemon info from disk.
    pub fn load(path: &Path) -> Option<Self> {
        let s = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&s).ok()
    }

    /// Remove the daemon info file.
    pub fn remove(path: &Path) {
        let _ = std::fs::remove_file(path);
    }

    /// Check if the daemon process is still alive.
    pub fn is_alive(&self) -> bool {
        pid_alive(self.pid)
    }
}

/// Check if a process with the given PID is alive.
#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    // kill(pid, 0) checks if process exists without sending a signal
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(not(unix))]
fn pid_alive(_pid: u32) -> bool {
    // Conservative: assume alive on non-Unix
    true
}

/// Try to find and start the `clanker-router` binary as a background daemon.
///
/// Returns the path to daemon.json if the daemon was started (or was already running).
pub fn auto_start_daemon() -> Option<PathBuf> {
    let info_path = daemon_info_path();

    // Check if already running
    if let Some(info) = DaemonInfo::load(&info_path) {
        if info.is_alive() {
            return Some(info_path);
        }
        // Stale pid file — clean up
        DaemonInfo::remove(&info_path);
    }

    // Find the clanker-router binary
    let bin = std::env::var("CLANKERS_ROUTER_BIN").ok().or_else(find_in_path)?;

    tracing::info!("Auto-starting router daemon: {}", bin);

    // Spawn as a detached background process.
    // We run `serve` (without --daemon) because --daemon does a re-exec;
    // we handle backgrounding here directly via null stdio + setsid.
    let mut cmd = std::process::Command::new(&bin);
    cmd.args(["serve"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    // Create a new session so the child survives terminal close / SIGHUP
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }

    let result = cmd.spawn();

    match result {
        Ok(_child) => {
            // Wait for daemon.json to appear (up to 5 seconds)
            for _ in 0..50 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if let Some(info) = DaemonInfo::load(&info_path)
                    && info.is_alive()
                {
                    tracing::info!("Router daemon started (pid {})", info.pid);
                    return Some(info_path);
                }
            }
            tracing::warn!("Router daemon started but daemon.json not found after 5s");
            None
        }
        Err(e) => {
            tracing::debug!("Failed to start router daemon: {}", e);
            None
        }
    }
}

/// Find `clanker-router` in PATH.
fn find_in_path() -> Option<String> {
    let path_var = std::env::var("PATH").ok()?;
    for dir in path_var.split(':') {
        let candidate = Path::new(dir).join("clanker-router");
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}
