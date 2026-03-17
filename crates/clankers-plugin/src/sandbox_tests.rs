//! Tests for the plugin sandbox permission model.

use super::sandbox::*;

// ── has_permission ──────────────────────────────────────────

// r[verify plugin.perm.all-grants-every]
#[test]
fn all_grants_every_permission() {
    let perms = vec!["all".to_string()];
    assert!(has_permission(&perms, Permission::FsRead));
    assert!(has_permission(&perms, Permission::FsWrite));
    assert!(has_permission(&perms, Permission::Net));
    assert!(has_permission(&perms, Permission::Exec));
    assert!(has_permission(&perms, Permission::Ui));
}

// r[verify plugin.perm.explicit-match]
#[test]
fn explicit_permission_match() {
    assert!(has_permission(&["fs:read".into()], Permission::FsRead));
    assert!(has_permission(&["fs:write".into()], Permission::FsWrite));
    assert!(has_permission(&["net".into()], Permission::Net));
    assert!(has_permission(&["exec".into()], Permission::Exec));
    assert!(has_permission(&["ui".into()], Permission::Ui));
}

// r[verify plugin.perm.deny-without-grant]
#[test]
fn empty_permissions_deny_all() {
    let perms: Vec<String> = vec![];
    assert!(!has_permission(&perms, Permission::FsRead));
    assert!(!has_permission(&perms, Permission::FsWrite));
    assert!(!has_permission(&perms, Permission::Net));
    assert!(!has_permission(&perms, Permission::Exec));
    assert!(!has_permission(&perms, Permission::Ui));
}

// r[verify plugin.perm.no-cross-grant]
#[test]
fn no_cross_grant_between_permissions() {
    // fs:read does not grant fs:write and vice versa
    assert!(!has_permission(&["fs:read".into()], Permission::FsWrite));
    assert!(!has_permission(&["fs:write".into()], Permission::FsRead));

    // net does not grant exec and vice versa
    assert!(!has_permission(&["net".into()], Permission::Exec));
    assert!(!has_permission(&["exec".into()], Permission::Net));

    // ui does not grant fs:read
    assert!(!has_permission(&["ui".into()], Permission::FsRead));

    // fs:read does not grant ui
    assert!(!has_permission(&["fs:read".into()], Permission::Ui));
}

#[test]
fn multiple_permissions_work() {
    let perms = vec!["fs:read".into(), "net".into()];
    assert!(has_permission(&perms, Permission::FsRead));
    assert!(has_permission(&perms, Permission::Net));
    assert!(!has_permission(&perms, Permission::FsWrite));
    assert!(!has_permission(&perms, Permission::Exec));
    assert!(!has_permission(&perms, Permission::Ui));
}

#[test]
fn unrecognized_permission_strings_ignored() {
    let perms = vec!["bogus".into(), "filesystem".into()];
    assert!(!has_permission(&perms, Permission::FsRead));
    assert!(!has_permission(&perms, Permission::FsWrite));
}

// ── Permission::as_str round-trip ───────────────────────────

// r[verify plugin.perm.no-cross-grant]
#[test]
fn as_str_values_are_distinct() {
    let strs = [
        Permission::FsRead.as_str(),
        Permission::FsWrite.as_str(),
        Permission::Net.as_str(),
        Permission::Exec.as_str(),
        Permission::Ui.as_str(),
    ];
    // All distinct
    for i in 0..strs.len() {
        for j in (i + 1)..strs.len() {
            assert_ne!(strs[i], strs[j], "{} == {}", strs[i], strs[j]);
        }
        // None is "all"
        assert_ne!(strs[i], "all");
    }
}

// ── filter_ui_actions ───────────────────────────────────────

// r[verify plugin.filter.strips-without-ui]
#[test]
fn filter_strips_without_ui_permission() {
    let perms = vec!["fs:read".into(), "net".into()];
    let actions = vec![1, 2, 3]; // dummy items
    let result = filter_ui_actions(&perms, actions);
    assert!(result.is_empty());
}

// r[verify plugin.filter.passes-with-ui]
#[test]
fn filter_passes_with_ui_permission() {
    let perms = vec!["ui".into()];
    let actions = vec![1, 2, 3];
    let result = filter_ui_actions(&perms, actions);
    assert_eq!(result.len(), 3);
}

// r[verify plugin.filter.passes-with-ui]
#[test]
fn filter_passes_with_all_permission() {
    let perms = vec!["all".into()];
    let actions = vec![42];
    let result = filter_ui_actions(&perms, actions);
    assert_eq!(result, vec![42]);
}

// r[verify plugin.filter.empty-passthrough]
#[test]
fn filter_empty_actions_passthrough() {
    let empty: Vec<i32> = vec![];
    // No permissions — empty in, empty out
    assert!(filter_ui_actions::<i32>(&[], empty.clone()).is_empty());
    // With ui — empty in, empty out
    assert!(filter_ui_actions::<i32>(&["ui".into()], empty).is_empty());
}

// ── check_tool_permission ───────────────────────────────────

#[test]
fn tool_permission_always_allowed() {
    // Tool invocation is always allowed; permission gates what the tool does
    assert!(check_tool_permission(&[], "any_tool").is_ok());
    assert!(check_tool_permission(&["fs:read".into()], "bash").is_ok());
}
