use std::path::Path;
use std::path::PathBuf;

pub(crate) const FORCE_RESTRICTED_UNAVAILABLE_ENV: &str = "CLANKERS_STDIO_FORCE_RESTRICTED_UNAVAILABLE";

pub(crate) const fn restricted_sandbox_supported_platform() -> bool {
    cfg!(target_os = "linux")
}

pub(crate) fn prepare_restricted_paths(writable_roots: &[PathBuf]) -> Result<(), String> {
    if let Some(message) = forced_restricted_unavailable_message() {
        return Err(message);
    }

    for root in writable_roots {
        std::fs::create_dir_all(root)
            .map_err(|error| format!("failed to prepare restricted writable root '{}': {}", root.display(), error))?;
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn apply_restricted_sandbox_to_current(
    _read_roots: &[PathBuf],
    _writable_roots: &[PathBuf],
    _allow_network: bool,
) -> Result<(), String> {
    Err("restricted sandbox mode is unavailable on this host".to_string())
}

#[cfg(target_os = "linux")]
pub(crate) fn apply_restricted_sandbox_to_current(
    read_roots: &[PathBuf],
    writable_roots: &[PathBuf],
    allow_network: bool,
) -> Result<(), String> {
    if let Some(message) = forced_restricted_unavailable_message() {
        return Err(message);
    }

    let merged_read_roots = merge_read_roots(read_roots);
    let landlock_supported = apply_landlock_rules(&merged_read_roots, writable_roots)?;
    if !landlock_supported {
        return Err("restricted sandbox mode is unavailable on this host (Landlock unsupported)".to_string());
    }

    if !allow_network {
        apply_socket_creation_filter()?;
    }

    Ok(())
}

fn forced_restricted_unavailable_message() -> Option<String> {
    std::env::var_os(FORCE_RESTRICTED_UNAVAILABLE_ENV).map(|_| {
        format!("restricted sandbox mode is unavailable on this host ({FORCE_RESTRICTED_UNAVAILABLE_ENV} is set)")
    })
}

#[cfg(target_os = "linux")]
fn merge_read_roots(extra_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for root in extra_roots {
        push_unique_path(&mut roots, root.clone());
    }
    for root in default_read_roots() {
        push_unique_path(&mut roots, root);
    }
    roots
}

#[cfg(target_os = "linux")]
fn default_read_roots() -> Vec<PathBuf> {
    let mut roots = vec![
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
        std::env::temp_dir(),
        PathBuf::from("/tmp"),
    ];

    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        roots.push(home.join(".cargo/bin"));
        roots.push(home.join(".cargo/registry"));
        roots.push(home.join(".rustup"));
        roots.push(home.join(".local"));
        roots.push(home.join(".nix-profile"));
    }

    roots
}

#[cfg(target_os = "linux")]
fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

#[cfg(target_os = "linux")]
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(function_length, reason = "sequential sandbox setup is clearer inline")
)]
fn apply_landlock_rules(read_roots: &[PathBuf], writable_roots: &[PathBuf]) -> Result<bool, String> {
    use std::os::unix::io::AsRawFd;

    const LANDLOCK_CREATE_RULESET: i64 = 444;
    const LANDLOCK_ADD_RULE: i64 = 445;
    const LANDLOCK_RESTRICT_SELF: i64 = 446;

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

    let attr = RulesetAttr {
        handled_access_fs: ALL_ACCESS,
        handled_access_net: 0,
    };
    let fd =
        unsafe { libc::syscall(LANDLOCK_CREATE_RULESET, &raw const attr, std::mem::size_of::<RulesetAttr>(), 0u32) };
    if fd < 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ENOSYS) || error.raw_os_error() == Some(libc::EOPNOTSUPP) {
            return Ok(false);
        }
        return Err(format!("landlock_create_ruleset: {}", error));
    }
    let fd = i32::try_from(fd).map_err(|_| "landlock fd out of i32 range".to_string())?;

    let add_rule = |path: &Path, access: u64| -> Result<(), String> {
        if !path.exists() {
            return Ok(());
        }
        let file = std::fs::File::open(path).map_err(|error| format!("open {}: {}", path.display(), error))?;
        let rule = PathBeneathAttr {
            allowed_access: access,
            parent_fd: file.as_raw_fd(),
        };
        let result = unsafe { libc::syscall(LANDLOCK_ADD_RULE, fd, RULE_PATH_BENEATH, &raw const rule, 0u32) };
        if result < 0 {
            return Err(format!("landlock_add_rule({}): {}", path.display(), std::io::Error::last_os_error()));
        }
        Ok(())
    };

    for root in writable_roots {
        add_rule(root, ALL_ACCESS)?;
    }
    for root in read_roots {
        add_rule(root, ALL_READ)?;
    }

    let no_new_privs = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if no_new_privs != 0 {
        unsafe {
            libc::close(fd);
        }
        return Err(format!("prctl(PR_SET_NO_NEW_PRIVS): {}", std::io::Error::last_os_error()));
    }

    let result = unsafe { libc::syscall(LANDLOCK_RESTRICT_SELF, fd, 0u32) };
    unsafe {
        libc::close(fd);
    }
    if result < 0 {
        Err(format!("landlock_restrict_self: {}", std::io::Error::last_os_error()))
    } else {
        Ok(true)
    }
}

#[cfg(target_os = "linux")]
fn apply_socket_creation_filter() -> Result<(), String> {
    const BPF_LD: u16 = 0x00;
    const BPF_W: u16 = 0x00;
    const BPF_ABS: u16 = 0x20;
    const BPF_JMP: u16 = 0x05;
    const BPF_JEQ: u16 = 0x10;
    const BPF_K: u16 = 0x00;
    const BPF_RET: u16 = 0x06;
    const SECCOMP_MODE_FILTER: libc::c_ulong = 2;
    const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
    const SECCOMP_RET_ERRNO: u32 = 0x0005_0000;

    let deny_errno = SECCOMP_RET_ERRNO | (libc::EPERM as u32);
    let filter = [
        libc::sock_filter {
            code: BPF_LD | BPF_W | BPF_ABS,
            jt: 0,
            jf: 0,
            k: 0,
        },
        libc::sock_filter {
            code: BPF_JMP | BPF_JEQ | BPF_K,
            jt: 0,
            jf: 1,
            k: libc::SYS_socket as u32,
        },
        libc::sock_filter {
            code: BPF_RET | BPF_K,
            jt: 0,
            jf: 0,
            k: deny_errno,
        },
        libc::sock_filter {
            code: BPF_JMP | BPF_JEQ | BPF_K,
            jt: 0,
            jf: 1,
            k: libc::SYS_socketpair as u32,
        },
        libc::sock_filter {
            code: BPF_RET | BPF_K,
            jt: 0,
            jf: 0,
            k: deny_errno,
        },
        libc::sock_filter {
            code: BPF_RET | BPF_K,
            jt: 0,
            jf: 0,
            k: SECCOMP_RET_ALLOW,
        },
    ];
    let program = libc::sock_fprog {
        len: u16::try_from(filter.len()).expect("filter len fits u16"),
        filter: filter.as_ptr().cast_mut(),
    };

    let no_new_privs = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if no_new_privs != 0 {
        return Err(format!("prctl(PR_SET_NO_NEW_PRIVS): {}", std::io::Error::last_os_error()));
    }

    let result = unsafe { libc::prctl(libc::PR_SET_SECCOMP, SECCOMP_MODE_FILTER, &raw const program) };
    if result != 0 {
        Err(format!("prctl(PR_SET_SECCOMP): {}", std::io::Error::last_os_error()))
    } else {
        Ok(())
    }
}
