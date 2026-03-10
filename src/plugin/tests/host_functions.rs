use std::collections::HashMap;

use crate::plugin::host::HostCall;
use crate::plugin::host::HostCallResult;
use crate::plugin::host::HostFunctions;

// ── log ──────────────────────────────────────────────────────────

#[test]
fn host_log_succeeds() {
    let hf = HostFunctions::new();
    let call = HostCall {
        function: "log".to_string(),
        args: serde_json::json!({"level": "info", "message": "test message"}),
    };
    let result = hf.execute(&call, &[]);
    assert!(result.ok);
}

#[test]
fn host_log_with_all_levels() {
    let hf = HostFunctions::new();
    for level in &["info", "warn", "error", "debug"] {
        let call = HostCall {
            function: "log".to_string(),
            args: serde_json::json!({"level": level, "message": "test"}),
        };
        let result = hf.execute(&call, &[]);
        assert!(result.ok, "log level '{}' should succeed", level);
    }
}

// ── get_config ───────────────────────────────────────────────────

#[test]
fn host_get_config_returns_value() {
    let mut config = HashMap::new();
    config.insert("api_url".to_string(), "https://example.com".to_string());
    let hf = HostFunctions::with_config(config);
    let call = HostCall {
        function: "get_config".to_string(),
        args: serde_json::json!({"key": "api_url"}),
    };
    let result = hf.execute(&call, &[]);
    assert!(result.ok);
    assert_eq!(result.data.as_str().unwrap(), "https://example.com");
}

#[test]
fn host_get_config_missing_key_returns_null() {
    let hf = HostFunctions::new();
    let call = HostCall {
        function: "get_config".to_string(),
        args: serde_json::json!({"key": "nonexistent"}),
    };
    let result = hf.execute(&call, &[]);
    assert!(result.ok);
    assert!(result.data.is_null());
}

// ── get_env ──────────────────────────────────────────────────────

#[test]
fn host_get_env_reads_env_var() {
    // PATH should exist on any system
    let hf = HostFunctions::new();
    let call = HostCall {
        function: "get_env".to_string(),
        args: serde_json::json!({"key": "PATH"}),
    };
    let result = hf.execute(&call, &[]);
    assert!(result.ok);
    assert!(result.data.is_string());
}

#[test]
fn host_get_env_missing_var_returns_null() {
    let hf = HostFunctions::new();
    let call = HostCall {
        function: "get_env".to_string(),
        args: serde_json::json!({"key": "CLANKERS_NONEXISTENT_VAR_12345"}),
    };
    let result = hf.execute(&call, &[]);
    assert!(result.ok);
    assert!(result.data.is_null());
}

#[test]
fn host_get_env_empty_key_returns_error() {
    let hf = HostFunctions::new();
    let call = HostCall {
        function: "get_env".to_string(),
        args: serde_json::json!({}),
    };
    let result = hf.execute(&call, &[]);
    assert!(!result.ok);
}

// ── read_file ────────────────────────────────────────────────────

#[test]
fn host_read_file_requires_fs_read_permission() {
    let hf = HostFunctions::new();
    let call = HostCall {
        function: "read_file".to_string(),
        args: serde_json::json!({"path": "/etc/hostname"}),
    };
    // No permissions → denied
    let result = hf.execute(&call, &[]);
    assert!(!result.ok);
    assert!(result.data.as_str().unwrap().contains("Permission denied"));
}

#[test]
fn host_read_file_works_with_permission() {
    let hf = HostFunctions::new();
    let call = HostCall {
        function: "read_file".to_string(),
        args: serde_json::json!({"path": "Cargo.toml"}),
    };
    let perms = vec!["fs:read".to_string()];
    let result = hf.execute(&call, &perms);
    assert!(result.ok, "Should read Cargo.toml: {:?}", result.data);
    assert!(result.data.as_str().unwrap().contains("[package]"));
}

#[test]
fn host_read_file_all_permission_works() {
    let hf = HostFunctions::new();
    let call = HostCall {
        function: "read_file".to_string(),
        args: serde_json::json!({"path": "Cargo.toml"}),
    };
    let perms = vec!["all".to_string()];
    let result = hf.execute(&call, &perms);
    assert!(result.ok);
}

#[test]
fn host_read_file_nonexistent_path() {
    let hf = HostFunctions::new();
    let call = HostCall {
        function: "read_file".to_string(),
        args: serde_json::json!({"path": "/tmp/clankers-nonexistent-file-xyz"}),
    };
    let perms = vec!["fs:read".to_string()];
    let result = hf.execute(&call, &perms);
    assert!(!result.ok);
}

#[test]
fn host_read_file_empty_path() {
    let hf = HostFunctions::new();
    let call = HostCall {
        function: "read_file".to_string(),
        args: serde_json::json!({}),
    };
    let perms = vec!["fs:read".to_string()];
    let result = hf.execute(&call, &perms);
    assert!(!result.ok);
}

// ── write_file ───────────────────────────────────────────────────

#[test]
fn host_write_file_requires_fs_write_permission() {
    let hf = HostFunctions::new();
    let call = HostCall {
        function: "write_file".to_string(),
        args: serde_json::json!({"path": "/tmp/test", "content": "hello"}),
    };
    let result = hf.execute(&call, &[]);
    assert!(!result.ok);
    assert!(result.data.as_str().unwrap().contains("Permission denied"));
}

#[test]
fn host_write_file_works_with_permission() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.txt");
    let hf = HostFunctions::new();
    let call = HostCall {
        function: "write_file".to_string(),
        args: serde_json::json!({"path": path.to_str().unwrap(), "content": "hello world"}),
    };
    let perms = vec!["fs:write".to_string()];
    let result = hf.execute(&call, &perms);
    assert!(result.ok, "write_file should succeed: {:?}", result.data);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello world");
}

// ── list_dir ─────────────────────────────────────────────────────

#[test]
fn host_list_dir_requires_fs_read_permission() {
    let hf = HostFunctions::new();
    let call = HostCall {
        function: "list_dir".to_string(),
        args: serde_json::json!({"path": "."}),
    };
    let result = hf.execute(&call, &[]);
    assert!(!result.ok);
    assert!(result.data.as_str().unwrap().contains("Permission denied"));
}

#[test]
fn host_list_dir_works_with_permission() {
    let hf = HostFunctions::new();
    let call = HostCall {
        function: "list_dir".to_string(),
        args: serde_json::json!({"path": "."}),
    };
    let perms = vec!["fs:read".to_string()];
    let result = hf.execute(&call, &perms);
    assert!(result.ok);
    let entries = result.data.as_array().unwrap();
    assert!(!entries.is_empty());
    // Should contain at least Cargo.toml
    let names: Vec<&str> = entries.iter().filter_map(|e| e.get("name").and_then(|n| n.as_str())).collect();
    assert!(names.contains(&"Cargo.toml"), "Should list Cargo.toml: {:?}", names);
}

// ── unknown function ─────────────────────────────────────────────

#[test]
fn host_unknown_function_returns_error() {
    let hf = HostFunctions::new();
    let call = HostCall {
        function: "nonexistent_fn".to_string(),
        args: serde_json::json!({}),
    };
    let result = hf.execute(&call, &["all".to_string()]);
    assert!(!result.ok);
    assert!(result.data.as_str().unwrap().contains("Unknown host function"));
}

// ── process_host_calls ───────────────────────────────────────────

#[test]
fn process_host_calls_extracts_from_response() {
    let hf = HostFunctions::new();
    let response = serde_json::json!({
        "tool": "my_tool",
        "result": "ok",
        "status": "ok",
        "host_calls": [
            {"fn": "log", "args": {"level": "info", "message": "hello"}},
            {"fn": "get_config", "args": {"key": "test"}}
        ]
    });
    let results = hf.process_host_calls(&response, &[]);
    assert_eq!(results.len(), 2);
    assert!(results[0].ok);
    assert!(results[1].ok);
}

#[test]
fn process_host_calls_empty_when_no_key() {
    let hf = HostFunctions::new();
    let response = serde_json::json!({"tool": "my_tool", "result": "ok"});
    let results = hf.process_host_calls(&response, &[]);
    assert!(results.is_empty());
}

#[test]
fn process_host_calls_malformed_entry() {
    let hf = HostFunctions::new();
    let response = serde_json::json!({
        "host_calls": [
            {"not_fn": "oops"}
        ]
    });
    let results = hf.process_host_calls(&response, &[]);
    assert_eq!(results.len(), 1);
    assert!(!results[0].ok);
}

// ── HostCallResult constructors ──────────────────────────────────

#[test]
fn host_call_result_success() {
    let r = HostCallResult::success(serde_json::json!("data"));
    assert!(r.ok);
    assert_eq!(r.data, serde_json::json!("data"));
}

#[test]
fn host_call_result_error() {
    let r = HostCallResult::error("something broke");
    assert!(!r.ok);
    assert_eq!(r.data.as_str().unwrap(), "something broke");
}
