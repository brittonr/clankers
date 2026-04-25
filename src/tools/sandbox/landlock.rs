//! Landlock filesystem access control for Linux
//!
//! Applies kernel-level restrictions to bash child processes, confining them
//! to specific read/write and read-only paths.

use std::path::Path;
use std::path::PathBuf;

/// Apply Landlock filesystem restrictions to the *current thread/process*.
///
/// Designed to be called inside a `pre_exec` hook on bash child processes,
/// NOT on the clankers parent. This way clankers itself remains unrestricted but
/// every shell command the agent runs is kernel-sandboxed.
///
/// `project_root` gets read-write; system paths get read-only.
///
/// Returns `Ok(true)` if applied, `Ok(false)` if unsupported, `Err` on failure.
#[cfg(target_os = "linux")]
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        function_length,
        reason = "sequential setup/dispatch logic — splitting would fragment readability"
    )
)]
pub fn apply_landlock_to_current(project_root: &Path) -> Result<bool, String> {
    use std::os::unix::io::AsRawFd;

    // Landlock syscall numbers
    const LANDLOCK_CREATE_RULESET: i64 = 444;
    const LANDLOCK_ADD_RULE: i64 = 445;
    const LANDLOCK_RESTRICT_SELF: i64 = 446;

    // ABI v1 access flags
    const FS_EXECUTE: u64 = 1 << 0;
    const FS_WRITE_FILE: u64 = 1 << 1;
    const FS_READ_FILE: u64 = 1 << 2;
    const FS_READ_DIR: u64 = 1 << 3;
    const FS_REMOVE_DIR: u64 = 1 << 4;
    const FS_REMOVE_FILE: u64 = 1 << 5;
    const FS_MAKE_CHAR: u64 = 1 << 6;
    const FS_MAKE_DIR: u64 = 1 << 7;
    const FS_MAKE_REG: u64 = 1 << 8;
    const FS_MAKE_SOCK: u64 = 1 << 9;
    const FS_MAKE_FIFO: u64 = 1 << 10;
    const FS_MAKE_BLOCK: u64 = 1 << 11;
    const FS_MAKE_SYM: u64 = 1 << 12;

    const RULE_PATH_BENEATH: i32 = 1;

    const ALL_READ: u64 = FS_EXECUTE | FS_READ_FILE | FS_READ_DIR;
    const ALL_WRITE: u64 = FS_WRITE_FILE
        | FS_REMOVE_DIR
        | FS_REMOVE_FILE
        | FS_MAKE_CHAR
        | FS_MAKE_DIR
        | FS_MAKE_REG
        | FS_MAKE_SOCK
        | FS_MAKE_FIFO
        | FS_MAKE_BLOCK
        | FS_MAKE_SYM;
    const ALL_ACCESS: u64 = ALL_READ | ALL_WRITE;

    #[repr(C)]
    struct RulesetAttr {
        handled_access_fs: u64,
        handled_access_net: u64,
    }

    #[repr(C)]
    struct PathBeneathAttr {
        allowed_access: u64,
        parent_fd: i32,
    }

    // Create ruleset
    let attr = RulesetAttr {
        handled_access_fs: ALL_ACCESS,
        handled_access_net: 0,
    };
    let fd =
        unsafe { libc::syscall(LANDLOCK_CREATE_RULESET, &raw const attr, std::mem::size_of::<RulesetAttr>(), 0u32) };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ENOSYS) || err.raw_os_error() == Some(libc::EOPNOTSUPP) {
            return Ok(false); // kernel doesn't support landlock
        }
        return Err(format!("landlock_create_ruleset: {}", err));
    }
    let fd = i32::try_from(fd).map_err(|_| "landlock fd out of i32 range".to_string())?;

    // Helper: add a path rule
    let add_rule = |path: &Path, access: u64| -> Result<(), String> {
        if !path.exists() {
            return Ok(());
        }
        let file = std::fs::File::open(path).map_err(|e| format!("open {}: {}", path.display(), e))?;
        let rule = PathBeneathAttr {
            allowed_access: access,
            parent_fd: file.as_raw_fd(),
        };
        let ret = unsafe { libc::syscall(LANDLOCK_ADD_RULE, fd, RULE_PATH_BENEATH, &raw const rule, 0u32) };
        // Keep file open until after syscall (fd must be valid)
        std::mem::forget(file);
        if ret < 0 {
            return Err(format!("landlock_add_rule({}): {}", path.display(), std::io::Error::last_os_error()));
        }
        Ok(())
    };

    // Read-write paths
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/homeless"));
    let rw_paths = [project_root.to_path_buf(), std::env::temp_dir(), PathBuf::from("/tmp")];
    for p in &rw_paths {
        add_rule(p, ALL_ACCESS).ok();
    }

    // Write access for nix daemon socket (nix build talks to the daemon via Unix socket)
    let nix_rw_paths = [
        PathBuf::from("/nix/var/nix/daemon-socket"),
        home.join(".cache/nix"),
        home.join(".local/state/nix"),
    ];
    for p in &nix_rw_paths {
        add_rule(p, ALL_ACCESS).ok();
    }

    // Read-only paths (system, toolchains)
    let ro_paths = [
        PathBuf::from("/nix"),
        PathBuf::from("/usr"),
        PathBuf::from("/lib"),
        PathBuf::from("/lib64"),
        PathBuf::from("/bin"),
        PathBuf::from("/sbin"),
        PathBuf::from("/etc"),
        PathBuf::from("/dev"),
        PathBuf::from("/proc"),
        PathBuf::from("/sys"),
        PathBuf::from("/run"),
        home.join(".cargo/bin"),
        home.join(".cargo/config.toml"),
        home.join(".cargo/registry"),
        home.join(".rustup"),
        home.join(".local"),
        home.join(".nix-profile"),
    ];
    for p in &ro_paths {
        add_rule(p, ALL_READ).ok();
    }

    // Restrict
    unsafe {
        libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0);
    }
    let ret = unsafe { libc::syscall(LANDLOCK_RESTRICT_SELF, fd, 0u32) };
    unsafe {
        libc::close(fd);
    }

    if ret < 0 {
        Err(format!("landlock_restrict_self: {}", std::io::Error::last_os_error()))
    } else {
        Ok(true)
    }
}

#[cfg(not(target_os = "linux"))]
pub fn apply_landlock_to_current(_project_root: &Path) -> Result<bool, String> {
    Ok(false)
}
