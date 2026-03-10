use crate::plugin::sandbox::Permission;
use crate::plugin::sandbox::filter_ui_actions;
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

#[test]
fn permission_as_str() {
    assert_eq!(Permission::FsRead.as_str(), "fs:read");
    assert_eq!(Permission::FsWrite.as_str(), "fs:write");
    assert_eq!(Permission::Net.as_str(), "net");
    assert_eq!(Permission::Exec.as_str(), "exec");
    assert_eq!(Permission::Ui.as_str(), "ui");
}

// ── UI action filtering ──────────────────────────────────────────

#[test]
fn filter_ui_actions_allows_with_ui_permission() {
    let perms = vec!["ui".to_string()];
    let actions = vec!["action1", "action2", "action3"];
    let result = filter_ui_actions(&perms, actions);
    assert_eq!(result.len(), 3);
}

#[test]
fn filter_ui_actions_allows_with_all_permission() {
    let perms = vec!["all".to_string()];
    let actions = vec!["action1"];
    let result = filter_ui_actions(&perms, actions);
    assert_eq!(result.len(), 1);
}

#[test]
fn filter_ui_actions_strips_without_ui_permission() {
    let perms = vec!["fs:read".to_string(), "net".to_string()];
    let actions = vec!["action1", "action2"];
    let result = filter_ui_actions(&perms, actions);
    assert!(result.is_empty());
}

#[test]
fn filter_ui_actions_empty_input_passes_through() {
    let perms: Vec<String> = vec![];
    let actions: Vec<String> = vec![];
    let result = filter_ui_actions(&perms, actions);
    assert!(result.is_empty());
}
