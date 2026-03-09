use crate::plugin::sandbox::Permission;
use crate::plugin::sandbox::has_permission;

// ── Sandbox permission tests ─────────────────────────────────────

#[test]
fn sandbox_permission_check() {
    let perms = vec!["fs:read".to_string(), "net".to_string()];
    assert!(has_permission(&perms, Permission::FsRead));
    assert!(has_permission(&perms, Permission::Net));
    assert!(!has_permission(&perms, Permission::FsWrite));
    assert!(!has_permission(&perms, Permission::Exec));
}

#[test]
fn sandbox_all_permission_grants_everything() {
    let perms = vec!["all".to_string()];
    assert!(has_permission(&perms, Permission::FsRead));
    assert!(has_permission(&perms, Permission::FsWrite));
    assert!(has_permission(&perms, Permission::Net));
    assert!(has_permission(&perms, Permission::Exec));
}

#[test]
fn sandbox_empty_permissions_deny_everything() {
    let perms: Vec<String> = vec![];
    assert!(!has_permission(&perms, Permission::FsRead));
    assert!(!has_permission(&perms, Permission::Net));
}

#[test]
fn sandbox_ui_permission() {
    let perms = vec!["ui".to_string()];
    assert!(has_permission(&perms, Permission::Ui));
    assert!(!has_permission(&perms, Permission::FsRead));

    let no_perms: Vec<String> = vec![];
    assert!(!has_permission(&no_perms, Permission::Ui));

    let all_perms = vec!["all".to_string()];
    assert!(has_permission(&all_perms, Permission::Ui));
}
