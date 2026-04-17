use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::OnceLock;
use std::time::Duration;

use tempfile::tempdir;

use crate::plugin::PluginRuntimeMode;
use crate::plugin::PluginState;

const HELPER_SOURCE: &str = r#"
use std::env;
use std::fs;
use std::io::{self, Read, Write};

fn read_frame() -> io::Result<String> {
    let mut len_buf = [0u8; 4];
    io::stdin().read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    io::stdin().read_exact(&mut payload)?;
    String::from_utf8(payload).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn write_frame(json: &str) -> io::Result<()> {
    let payload = json.as_bytes();
    io::stdout().write_all(&(payload.len() as u32).to_be_bytes())?;
    io::stdout().write_all(payload)?;
    io::stdout().flush()
}

fn json_escape(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn counter(path: &str) -> u32 {
    if path.is_empty() {
        return 1;
    }
    let current = fs::read_to_string(path)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(0)
        + 1;
    fs::write(path, current.to_string()).ok();
    current
}

fn send_startup(plugin: &str, tool: &str) -> io::Result<()> {
    write_frame(&format!(
        "{{\"type\":\"hello\",\"plugin_protocol\":1,\"plugin\":\"{}\",\"version\":\"0.1.0\"}}",
        json_escape(plugin)
    ))?;
    write_frame("{\"type\":\"ready\",\"plugin_protocol\":1}")?;
    write_frame(&format!(
        "{{\"type\":\"register_tools\",\"plugin_protocol\":1,\"tools\":[{{\"name\":\"{}\",\"description\":\"test tool\",\"input_schema\":{{\"type\":\"object\"}}}}]}}",
        json_escape(tool)
    ))?;
    write_frame("{\"type\":\"subscribe_events\",\"plugin_protocol\":1,\"events\":[\"tool_call\"]}")
}

fn wait_for_shutdown(shutdown_file: &str) {
    loop {
        match read_frame() {
            Ok(frame) => {
                if frame.contains("\"type\":\"shutdown\"") {
                    if !shutdown_file.is_empty() {
                        fs::write(shutdown_file, "shutdown").ok();
                    }
                    return;
                }
            }
            Err(_) => return,
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let behavior = args.get(1).cloned().unwrap_or_else(|| "ready_register".to_string());
    let plugin = args.get(2).cloned().unwrap_or_else(|| "stdio-test".to_string());
    let tool = args.get(3).cloned().unwrap_or_else(|| "stdio_tool".to_string());
    let expected_mode = args.get(4).cloned().unwrap_or_default();
    let state_file = args.get(5).cloned().unwrap_or_default();
    let shutdown_file = args.get(6).cloned().unwrap_or_default();

    if behavior == "bad_handshake" {
        write_frame("{\"type\":\"ready\",\"plugin_protocol\":1}").ok();
        return;
    }
    if behavior == "stderr_bad_handshake" {
        eprintln!("helper stderr launch diagnostic");
        write_frame("{\"type\":\"ready\",\"plugin_protocol\":1}").ok();
        return;
    }

    let host_hello = read_frame().expect("read host hello");
    if !host_hello.contains("\"type\":\"hello\"") {
        eprintln!("expected host hello, got: {}", host_hello);
        std::process::exit(11);
    }
    if !expected_mode.is_empty() && !host_hello.contains(&format!("\"mode\":\"{}\"", expected_mode)) {
        eprintln!("mode mismatch: {}", host_hello);
        std::process::exit(12);
    }
    if !host_hello.contains(&format!("\"plugin\":\"{}\"", json_escape(&plugin))) {
        eprintln!("plugin mismatch: {}", host_hello);
        std::process::exit(13);
    }

    match behavior.as_str() {
        "ready_register" | "ready_then_wait_shutdown" => {
            send_startup(&plugin, &tool).expect("send startup frames");
            wait_for_shutdown(&shutdown_file);
        }
        "ready_then_exit" => {
            send_startup(&plugin, &tool).expect("send startup frames");
            std::process::exit(19);
        }
        "ready_then_crash_once_then_register" => {
            let current = counter(&state_file);
            send_startup(&plugin, &tool).expect("send startup frames");
            if current == 1 {
                eprintln!("intentional crash after ready");
                std::process::exit(17);
            }
            wait_for_shutdown(&shutdown_file);
        }
        other => {
            eprintln!("unknown helper behavior: {}", other);
            std::process::exit(99);
        }
    }
}
"#;

struct RestartDelayGuard {
    _lock: MutexGuard<'static, ()>,
    previous: Option<String>,
}

impl RestartDelayGuard {
    fn set(value: &str) -> Self {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let lock = LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::var("CLANKERS_STDIO_RESTART_DELAYS_MS").ok();
        unsafe {
            std::env::set_var("CLANKERS_STDIO_RESTART_DELAYS_MS", value);
        }
        Self {
            _lock: lock,
            previous,
        }
    }
}

impl Drop for RestartDelayGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            unsafe {
                std::env::set_var("CLANKERS_STDIO_RESTART_DELAYS_MS", previous);
            }
        } else {
            unsafe {
                std::env::remove_var("CLANKERS_STDIO_RESTART_DELAYS_MS");
            }
        }
    }
}

fn helper_binary() -> &'static PathBuf {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let base = std::env::temp_dir().join("clankers-stdio-helper-shared");
        std::fs::create_dir_all(&base).unwrap();
        let source = base.join("helper.rs");
        let binary = base.join("helper-bin");
        let needs_rebuild = std::fs::read_to_string(&source).ok().as_deref() != Some(HELPER_SOURCE) || !binary.exists();
        if needs_rebuild {
            std::fs::write(&source, HELPER_SOURCE).unwrap();
            let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
            let output = std::process::Command::new(rustc)
                .arg("--edition=2024")
                .arg(&source)
                .arg("-O")
                .arg("-o")
                .arg(&binary)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "failed to compile stdio helper: stdout={} stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        binary
    })
}

fn write_stdio_plugin_manifest(
    dir: &Path,
    name: &str,
    behavior: &str,
    expected_mode: &str,
    state_file: Option<&Path>,
    shutdown_file: Option<&Path>,
) {
    let plugin_dir = dir.join(name);
    std::fs::create_dir_all(&plugin_dir).unwrap();
    let helper = helper_binary();
    let tool_name = format!("{}_tool", name.replace('-', "_"));
    let args = serde_json::json!([
        behavior,
        name,
        tool_name,
        expected_mode,
        state_file.map(|path| path.display().to_string()).unwrap_or_default(),
        shutdown_file.map(|path| path.display().to_string()).unwrap_or_default()
    ]);
    std::fs::write(
        plugin_dir.join("plugin.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "name": name,
            "version": "0.1.0",
            "kind": "stdio",
            "permissions": ["ui"],
            "stdio": {
                "command": helper.display().to_string(),
                "args": args,
                "working_dir": "plugin-dir",
                "sandbox": "inherit"
            }
        }))
        .unwrap(),
    )
    .unwrap();
}

async fn wait_for_plugin_state(
    manager: &Arc<Mutex<crate::plugin::PluginManager>>,
    name: &str,
    timeout: Duration,
    predicate: impl Fn(&PluginState) -> bool,
) -> PluginState {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let state = {
            let manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            manager.get(name).expect("plugin present").state.clone()
        };
        if predicate(&state) {
            return state;
        }
        assert!(tokio::time::Instant::now() < deadline, "timed out waiting for plugin state; last={state:?}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn wait_for_live_tool(
    manager: &Arc<Mutex<crate::plugin::PluginManager>>,
    plugin_name: &str,
    tool_name: &str,
    timeout: Duration,
) {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let summaries = crate::plugin::build_protocol_plugin_summaries(manager);
        if summaries
            .iter()
            .find(|summary| summary.name == plugin_name)
            .is_some_and(|summary| summary.tools.iter().any(|tool| tool == tool_name))
        {
            return;
        }
        assert!(tokio::time::Instant::now() < deadline, "timed out waiting for live tool '{tool_name}'");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn sample_plugin_states(
    manager: &Arc<Mutex<crate::plugin::PluginManager>>,
    name: &str,
    duration: Duration,
    interval: Duration,
) -> Vec<PluginState> {
    let deadline = tokio::time::Instant::now() + duration;
    let mut states = Vec::new();
    loop {
        let state = {
            let manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            manager.get(name).expect("plugin present").state.clone()
        };
        states.push(state);
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(interval).await;
    }
    states
}

#[tokio::test]
async fn standalone_stdio_plugin_launches_and_registers_live_tools() {
    let _guard = RestartDelayGuard::set("5,10,15,20,25");
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-standalone",
        "ready_register",
        "standalone",
        None,
        None,
    );

    let manager = crate::modes::common::init_plugin_manager_for_mode(
        dir.path(),
        None,
        &[],
        PluginRuntimeMode::Standalone,
        dir.path(),
    );

    let state = wait_for_plugin_state(&manager, "stdio-standalone", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    assert_eq!(state, PluginState::Active);
    wait_for_live_tool(&manager, "stdio-standalone", "stdio_standalone_tool", Duration::from_secs(2)).await;

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn daemon_stdio_plugin_launches_with_daemon_mode_hello() {
    let _guard = RestartDelayGuard::set("5,10,15,20,25");
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-daemon",
        "ready_register",
        "daemon",
        None,
        None,
    );

    let manager = crate::modes::common::init_plugin_manager_for_mode(
        dir.path(),
        None,
        &[],
        PluginRuntimeMode::Daemon,
        dir.path(),
    );

    let state = wait_for_plugin_state(&manager, "stdio-daemon", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    assert_eq!(state, PluginState::Active);

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn invalid_stdio_handshake_enters_error_state() {
    let _guard = RestartDelayGuard::set("5,10,15,20,25");
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-bad-handshake",
        "bad_handshake",
        "",
        None,
        None,
    );

    let manager = crate::modes::common::init_plugin_manager_for_mode(
        dir.path(),
        None,
        &[],
        PluginRuntimeMode::Standalone,
        dir.path(),
    );

    let state = wait_for_plugin_state(&manager, "stdio-bad-handshake", Duration::from_secs(20), |state| {
        matches!(state, PluginState::Error(_))
    })
    .await;
    assert!(matches!(state, PluginState::Error(message) if message.contains("ready before hello")));
}

#[tokio::test]
async fn active_stdio_crash_enters_backoff_then_restarts() {
    let _guard = RestartDelayGuard::set("500,1000,1500,2000,2500");
    let dir = tempdir().unwrap();
    let counter = dir.path().join("launch-count.txt");
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-restart",
        "ready_then_crash_once_then_register",
        "standalone",
        Some(&counter),
        None,
    );

    let manager = crate::modes::common::init_plugin_manager_for_mode(
        dir.path(),
        None,
        &[],
        PluginRuntimeMode::Standalone,
        dir.path(),
    );

    let seen_states = sample_plugin_states(&manager, "stdio-restart", Duration::from_secs(2), Duration::from_millis(5)).await;
    assert!(
        seen_states.iter().any(|state| matches!(state, PluginState::Backoff(message) if message.contains("intentional crash"))),
        "expected backoff state in history: {seen_states:?}"
    );

    let state = wait_for_plugin_state(&manager, "stdio-restart", Duration::from_secs(5), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    assert_eq!(state, PluginState::Active);
    assert_eq!(std::fs::read_to_string(&counter).unwrap().trim(), "2");
    wait_for_live_tool(&manager, "stdio-restart", "stdio_restart_tool", Duration::from_secs(5)).await;

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn shutdown_sends_shutdown_frame_and_clears_live_tools() {
    let _guard = RestartDelayGuard::set("5,10,15,20,25");
    let dir = tempdir().unwrap();
    let shutdown_marker = dir.path().join("shutdown.txt");
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-shutdown",
        "ready_then_wait_shutdown",
        "standalone",
        None,
        Some(&shutdown_marker),
    );

    let manager = crate::modes::common::init_plugin_manager_for_mode(
        dir.path(),
        None,
        &[],
        PluginRuntimeMode::Standalone,
        dir.path(),
    );

    wait_for_plugin_state(&manager, "stdio-shutdown", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_live_tool(&manager, "stdio-shutdown", "stdio_shutdown_tool", Duration::from_secs(2)).await;

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;

    let state = wait_for_plugin_state(&manager, "stdio-shutdown", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Loaded)
    })
    .await;
    assert_eq!(state, PluginState::Loaded);
    assert_eq!(std::fs::read_to_string(&shutdown_marker).unwrap(), "shutdown");

    let summaries = crate::plugin::build_protocol_plugin_summaries(&manager);
    let plugin = summaries.iter().find(|summary| summary.name == "stdio-shutdown").unwrap();
    assert!(plugin.tools.is_empty(), "tools should be cleared on disconnect: {:?}", plugin.tools);
}

#[tokio::test]
async fn stderr_is_included_in_launch_diagnostics() {
    let _guard = RestartDelayGuard::set("5,10,15,20,25");
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-stderr",
        "stderr_bad_handshake",
        "",
        None,
        None,
    );

    let manager = crate::modes::common::init_plugin_manager_for_mode(
        dir.path(),
        None,
        &[],
        PluginRuntimeMode::Standalone,
        dir.path(),
    );

    let state = wait_for_plugin_state(&manager, "stdio-stderr", Duration::from_secs(20), |state| {
        matches!(state, PluginState::Error(_))
    })
    .await;
    assert!(matches!(state, PluginState::Error(message) if message.contains("helper stderr launch diagnostic")));
}
