//! Tests for host function permission gating.

use super::host::*;

fn make_call(function: &str) -> HostCall {
    HostCall {
        function: function.to_string(),
        args: serde_json::json!({}),
    }
}

// ── Ungated functions ───────────────────────────────────────

// r[verify plugin.host.ungated-functions]
#[test]
fn log_needs_no_permission() {
    let host = HostFunctions::new();
    let result = host.execute(
        &HostCall {
            function: "log".into(),
            args: serde_json::json!({"level": "info", "message": "test"}),
        },
        &[],
    );
    assert!(result.ok);
}

// r[verify plugin.host.ungated-functions]
#[test]
fn get_config_needs_no_permission() {
    let host = HostFunctions::with_config([("api_key".into(), "secret".into())].into());
    let result = host.execute(
        &HostCall {
            function: "get_config".into(),
            args: serde_json::json!({"key": "api_key"}),
        },
        &[],
    );
    assert!(result.ok);
    assert_eq!(result.data, serde_json::json!("secret"));
}

// r[verify plugin.host.ungated-functions]
#[test]
fn get_env_needs_no_permission() {
    let host = HostFunctions::new();
    // HOME is always set
    let result = host.execute(
        &HostCall {
            function: "get_env".into(),
            args: serde_json::json!({"key": "HOME"}),
        },
        &[],
    );
    assert!(result.ok);
}

// ── fs:read gated ───────────────────────────────────────────

// r[verify plugin.host.fs-read-gated]
#[test]
fn read_file_denied_without_permission() {
    let host = HostFunctions::new();
    let result = host.execute(&make_call("read_file"), &[]);
    assert!(!result.ok);
    assert!(result.data.as_str().unwrap().contains("Permission denied"));
}

// r[verify plugin.host.fs-read-gated]
#[test]
fn list_dir_denied_without_permission() {
    let host = HostFunctions::new();
    let result = host.execute(&make_call("list_dir"), &[]);
    assert!(!result.ok);
    assert!(result.data.as_str().unwrap().contains("Permission denied"));
}

// r[verify plugin.host.fs-read-gated]
#[test]
fn read_file_allowed_with_fs_read() {
    let host = HostFunctions::new();
    // Will fail on missing path, but permission check passes
    let result = host.execute(
        &HostCall {
            function: "read_file".into(),
            args: serde_json::json!({"path": "/nonexistent"}),
        },
        &["fs:read".into()],
    );
    // Permission passed, file doesn't exist
    assert!(!result.ok);
    assert!(result.data.as_str().unwrap().contains("Cannot read"));
}

// r[verify plugin.host.fs-read-gated]
#[test]
fn list_dir_allowed_with_all() {
    let host = HostFunctions::new();
    // "all" grants fs:read
    let result = host.execute(
        &HostCall {
            function: "list_dir".into(),
            args: serde_json::json!({"path": "."}),
        },
        &["all".into()],
    );
    assert!(result.ok);
}

// ── fs:write gated ──────────────────────────────────────────

// r[verify plugin.host.fs-write-gated]
#[test]
fn write_file_denied_without_permission() {
    let host = HostFunctions::new();
    let result = host.execute(&make_call("write_file"), &[]);
    assert!(!result.ok);
    assert!(result.data.as_str().unwrap().contains("Permission denied"));
}

// r[verify plugin.host.fs-write-gated]
#[test]
fn write_file_denied_with_only_fs_read() {
    let host = HostFunctions::new();
    // fs:read does NOT grant fs:write
    let result = host.execute(&make_call("write_file"), &["fs:read".into()]);
    assert!(!result.ok);
    assert!(result.data.as_str().unwrap().contains("Permission denied"));
}

// ── Unknown functions ───────────────────────────────────────

// r[verify plugin.host.unknown-rejects]
#[test]
fn unknown_function_rejected() {
    let host = HostFunctions::new();
    let result = host.execute(&make_call("exec_shell"), &["all".into()]);
    assert!(!result.ok);
    assert!(result.data.as_str().unwrap().contains("Unknown host function"));
}

// r[verify plugin.host.unknown-rejects]
#[test]
fn empty_function_name_rejected() {
    let host = HostFunctions::new();
    let result = host.execute(&make_call(""), &["all".into()]);
    assert!(!result.ok);
}

// ── process_host_calls ──────────────────────────────────────

#[test]
fn process_host_calls_mixed_permissions() {
    let host = HostFunctions::new();
    let response = serde_json::json!({
        "host_calls": [
            {"fn": "log", "args": {"message": "ok"}},
            {"fn": "read_file", "args": {"path": "/etc/hostname"}},
            {"fn": "write_file", "args": {"path": "/tmp/x", "content": "y"}}
        ]
    });

    // Only fs:read granted
    let results = host.process_host_calls(&response, &["fs:read".into()]);
    assert_eq!(results.len(), 3);
    assert!(results[0].ok); // log: ungated
    // read_file: fs:read granted, but depends on file existing
    // write_file: fs:write NOT granted
    assert!(!results[2].ok);
    assert!(results[2].data.as_str().unwrap().contains("Permission denied"));
}

#[test]
fn process_host_calls_no_calls_key() {
    let host = HostFunctions::new();
    let response = serde_json::json!({"result": "ok"});
    let results = host.process_host_calls(&response, &[]);
    assert!(results.is_empty());
}
