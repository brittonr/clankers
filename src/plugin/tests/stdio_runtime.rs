use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::OnceLock;
use std::time::Duration;

use tempfile::tempdir;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::agent::events::AgentEvent;
use crate::plugin::PluginRuntimeMode;
use crate::plugin::PluginState;
use crate::tools::ToolContext;

const HELPER_SOURCE: &str = r#"
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::net::TcpStream;

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

fn json_string_field(input: &str, field: &str) -> Option<String> {
    let needle = format!("\"{}\":\"", field);
    let start = input.find(&needle)? + needle.len();
    let rest = &input[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
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

fn env_or_unset(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| "<unset>".to_string())
}

fn record_launch_snapshot(path: &str) {
    if path.is_empty() {
        return;
    }
    let cwd = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let snapshot = format!(
        "cwd={}\nGITHUB_TOKEN={}\nFASTMAIL_TOKEN={}\nSHOULD_NOT_LEAK={}\nPATH={}\n",
        cwd.display(),
        env_or_unset("GITHUB_TOKEN"),
        env_or_unset("FASTMAIL_TOKEN"),
        env_or_unset("SHOULD_NOT_LEAK"),
        env_or_unset("PATH"),
    );
    fs::write(path, snapshot).ok();
}

fn send_tool_result(call_id: &str, content: &str) {
    write_frame(&format!(
        "{{\"type\":\"tool_result\",\"plugin_protocol\":1,\"call_id\":\"{}\",\"content\":\"{}\"}}",
        json_escape(call_id),
        json_escape(content)
    ))
    .ok();
}

fn write_probe(path_var: &str, label: &str) -> String {
    let path = env::var(path_var).unwrap_or_default();
    if path.is_empty() {
        return format!("{}=unset", label);
    }
    match fs::write(&path, label) {
        Ok(()) => format!("{}=ok", label),
        Err(error) => format!("{}=err:{}", label, error),
    }
}

fn network_probe() -> String {
    let addr = env::var("CLANKERS_TEST_NETWORK_ADDR").unwrap_or_default();
    if addr.is_empty() {
        return "network=unset".to_string();
    }
    match TcpStream::connect(&addr) {
        Ok(_) => "network=ok".to_string(),
        Err(error) => format!("network=err:{}", error),
    }
}

fn sandbox_probe_summary() -> String {
    [
        write_probe("CLANKERS_TEST_STATE_WRITE_PATH", "state_write"),
        write_probe("CLANKERS_TEST_ALLOWED_WRITE_PATH", "allowed_write"),
        write_probe("CLANKERS_TEST_DENIED_WRITE_PATH", "denied_write"),
        network_probe(),
    ]
    .join(";")
}

fn send_startup(plugin: &str, tool: &str) -> io::Result<()> {
    send_startup_with_events(plugin, tool, "[\"tool_call\"]")
}

fn send_startup_with_events(plugin: &str, tool: &str, events_json: &str) -> io::Result<()> {
    write_frame(&format!(
        "{{\"type\":\"hello\",\"plugin_protocol\":1,\"plugin\":\"{}\",\"version\":\"0.1.0\"}}",
        json_escape(plugin)
    ))?;
    write_frame("{\"type\":\"ready\",\"plugin_protocol\":1}")?;
    write_frame(&format!(
        "{{\"type\":\"register_tools\",\"plugin_protocol\":1,\"tools\":[{{\"name\":\"{}\",\"description\":\"test tool\",\"input_schema\":{{\"type\":\"object\"}}}}]}}",
        json_escape(tool)
    ))?;
    write_frame(&format!(
        "{{\"type\":\"subscribe_events\",\"plugin_protocol\":1,\"events\":{}}}",
        events_json
    ))
}

fn send_startup_with_builtin_collision(plugin: &str, tool: &str) -> io::Result<()> {
    write_frame(&format!(
        "{{\"type\":\"hello\",\"plugin_protocol\":1,\"plugin\":\"{}\",\"version\":\"0.1.0\"}}",
        json_escape(plugin)
    ))?;
    write_frame("{\"type\":\"ready\",\"plugin_protocol\":1}")?;
    write_frame(&format!(
        "{{\"type\":\"register_tools\",\"plugin_protocol\":1,\"tools\":[{{\"name\":\"read\",\"description\":\"builtin collision\",\"input_schema\":{{\"type\":\"object\"}}}},{{\"name\":\"{}\",\"description\":\"test tool\",\"input_schema\":{{\"type\":\"object\"}}}}]}}",
        json_escape(tool)
    ))?;
    write_frame("{\"type\":\"subscribe_events\",\"plugin_protocol\":1,\"events\":[\"tool_call\"]}")
}

fn send_startup_with_extism_collision(plugin: &str, tool: &str) -> io::Result<()> {
    let extra_tool = format!("{}_extra", tool);
    write_frame(&format!(
        "{{\"type\":\"hello\",\"plugin_protocol\":1,\"plugin\":\"{}\",\"version\":\"0.1.0\"}}",
        json_escape(plugin)
    ))?;
    write_frame("{\"type\":\"ready\",\"plugin_protocol\":1}")?;
    write_frame(&format!(
        "{{\"type\":\"register_tools\",\"plugin_protocol\":1,\"tools\":[{{\"name\":\"test_echo\",\"description\":\"extism collision\",\"input_schema\":{{\"type\":\"object\"}}}},{{\"name\":\"{}\",\"description\":\"test tool\",\"input_schema\":{{\"type\":\"object\"}}}}]}}",
        json_escape(&extra_tool)
    ))?;
    write_frame("{\"type\":\"subscribe_events\",\"plugin_protocol\":1,\"events\":[\"tool_call\"]}")
}

fn emit_event_ui_display(plugin: &str, tool_name: &str) {
    write_frame(&format!(
        "{{\"type\":\"display\",\"plugin_protocol\":1,\"message\":\"saw tool_call for {}\"}}",
        json_escape(tool_name)
    )).ok();
    let ui = format!(
        "{{\"type\":\"ui\",\"plugin_protocol\":1,\"actions\":[{{\"action\":\"set_status\",\"plugin\":\"{}\",\"text\":\"tool {}\",\"color\":\"green\"}},{{\"action\":\"notify\",\"plugin\":\"{}\",\"message\":\"note {}\",\"level\":\"info\"}},{{\"action\":\"set_widget\",\"plugin\":\"{}\",\"widget\":{{\"type\":\"Text\",\"content\":\"widget {}\",\"bold\":false,\"color\":null}}}}]}}",
        json_escape(plugin),
        json_escape(tool_name),
        json_escape(plugin),
        json_escape(tool_name),
        json_escape(plugin),
        json_escape(tool_name),
    );
    write_frame(&ui).ok();
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
        std::thread::sleep(std::time::Duration::from_millis(20));
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
        "ready_register_with_builtin_collision" => {
            send_startup_with_builtin_collision(&plugin, &tool).expect("send startup frames");
            wait_for_shutdown(&shutdown_file);
        }
        "ready_register_with_extism_collision" => {
            send_startup_with_extism_collision(&plugin, &tool).expect("send startup frames");
            wait_for_shutdown(&shutdown_file);
        }
        "count_launches_and_wait_shutdown" => {
            counter(&state_file);
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
        "bad_handshake_until_sixth_launch_then_register" => {
            let current = counter(&state_file);
            if current <= 5 {
                write_frame("{\"type\":\"ready\",\"plugin_protocol\":1}").ok();
                return;
            }
            send_startup(&plugin, &tool).expect("send startup frames");
            wait_for_shutdown(&shutdown_file);
        }
        "four_bad_then_ready_then_bad_then_register" => {
            let current = counter(&state_file);
            match current {
                1..=4 => {
                    write_frame("{\"type\":\"ready\",\"plugin_protocol\":1}").ok();
                    return;
                }
                5 => {
                    send_startup(&plugin, &tool).expect("send startup frames");
                    eprintln!("intentional crash after ready reset point");
                    std::process::exit(23);
                }
                6 => {
                    write_frame("{\"type\":\"ready\",\"plugin_protocol\":1}").ok();
                    return;
                }
                _ => {
                    send_startup(&plugin, &tool).expect("send startup frames");
                    wait_for_shutdown(&shutdown_file);
                }
            }
        }
        "record_env_and_cwd" => {
            record_launch_snapshot(&state_file);
            send_startup(&plugin, &tool).expect("send startup frames");
            wait_for_shutdown(&shutdown_file);
        }
        "invoke_probe_sandbox" => {
            send_startup(&plugin, &tool).expect("send startup frames");
            loop {
                let frame = match read_frame() {
                    Ok(frame) => frame,
                    Err(_) => return,
                };
                if frame.contains("\"type\":\"tool_invoke\"") {
                    let call_id = json_string_field(&frame, "call_id").expect("call_id in invoke");
                    send_tool_result(&call_id, &sandbox_probe_summary());
                } else if frame.contains("\"type\":\"shutdown\"") {
                    return;
                }
            }
        }
        "register_tools_additive" => {
            send_startup(&plugin, &tool).expect("send startup frames");
            write_frame(&format!(
                "{{\"type\":\"register_tools\",\"plugin_protocol\":1,\"tools\":[{{\"name\":\"{}_extra\",\"description\":\"extra tool\",\"input_schema\":{{\"type\":\"object\"}}}}]}}",
                json_escape(&tool)
            )).ok();
            wait_for_shutdown(&shutdown_file);
        }
        "subscribe_then_crash_then_do_not_resubscribe" => {
            let current = counter(&state_file);
            if current == 1 {
                send_startup_with_events(&plugin, &tool, "[\"tool_call\"]").expect("send startup frames");
                eprintln!("intentional crash after subscribed ready");
                std::process::exit(24);
            }
            send_startup_with_events(&plugin, &tool, "[]").expect("send startup frames");
            loop {
                let frame = match read_frame() {
                    Ok(frame) => frame,
                    Err(_) => return,
                };
                if frame.contains("\"type\":\"event\"") {
                    write_frame("{\"type\":\"display\",\"plugin_protocol\":1,\"message\":\"unexpected event after restart\"}").ok();
                } else if frame.contains("\"type\":\"shutdown\"") {
                    return;
                }
            }
        }
        "invoke_success" => {
            send_startup(&plugin, &tool).expect("send startup frames");
            loop {
                let frame = match read_frame() {
                    Ok(frame) => frame,
                    Err(_) => return,
                };
                if frame.contains("\"type\":\"tool_invoke\"") {
                    let call_id = json_string_field(&frame, "call_id").expect("call_id in invoke");
                    write_frame(&format!(
                        "{{\"type\":\"tool_progress\",\"plugin_protocol\":1,\"call_id\":\"{}\",\"message\":\"helper progress\"}}",
                        json_escape(&call_id)
                    )).ok();
                    write_frame(&format!(
                        "{{\"type\":\"tool_result\",\"plugin_protocol\":1,\"call_id\":\"{}\",\"content\":\"helper result\"}}",
                        json_escape(&call_id)
                    )).ok();
                } else if frame.contains("\"type\":\"shutdown\"") {
                    return;
                }
            }
        }
        "invoke_wait_for_cancel" => {
            send_startup(&plugin, &tool).expect("send startup frames");
            let mut active_call_id = None::<String>;
            loop {
                let frame = match read_frame() {
                    Ok(frame) => frame,
                    Err(_) => return,
                };
                if frame.contains("\"type\":\"tool_invoke\"") {
                    active_call_id = json_string_field(&frame, "call_id");
                } else if frame.contains("\"type\":\"tool_cancel\"") {
                    let call_id = json_string_field(&frame, "call_id").expect("call_id in cancel");
                    if active_call_id.as_deref() == Some(call_id.as_str()) {
                        write_frame(&format!(
                            "{{\"type\":\"tool_cancelled\",\"plugin_protocol\":1,\"call_id\":\"{}\"}}",
                            json_escape(&call_id)
                        )).ok();
                    }
                } else if frame.contains("\"type\":\"shutdown\"") {
                    return;
                }
            }
        }
        "invoke_never_finishes" => {
            send_startup(&plugin, &tool).expect("send startup frames");
            wait_for_shutdown(&shutdown_file);
        }
        "ignore_shutdown_forever" => {
            send_startup(&plugin, &tool).expect("send startup frames");
            loop {
                let frame = match read_frame() {
                    Ok(frame) => frame,
                    Err(_) => return,
                };
                if frame.contains("\"type\":\"shutdown\"") {
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                    }
                }
            }
        }
        "event_tool_call_ui_display" => {
            send_startup_with_events(&plugin, &tool, "[\"tool_call\"]").expect("send startup frames");
            loop {
                let frame = match read_frame() {
                    Ok(frame) => frame,
                    Err(_) => return,
                };
                if frame.contains("\"type\":\"event\"") && frame.contains("\"name\":\"tool_call\"") {
                    let seen_tool = json_string_field(&frame, "tool").unwrap_or_else(|| "unknown".to_string());
                    emit_event_ui_display(&plugin, &seen_tool);
                } else if frame.contains("\"type\":\"shutdown\"") {
                    return;
                }
            }
        }
        "event_unsubscribed_emit_if_called" => {
            send_startup_with_events(&plugin, &tool, "[\"message_update\"]").expect("send startup frames");
            loop {
                let frame = match read_frame() {
                    Ok(frame) => frame,
                    Err(_) => return,
                };
                if frame.contains("\"type\":\"event\"") {
                    write_frame("{\"type\":\"display\",\"plugin_protocol\":1,\"message\":\"unexpected event\"}").ok();
                } else if frame.contains("\"type\":\"shutdown\"") {
                    return;
                }
            }
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
        let lock = LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::var("CLANKERS_STDIO_RESTART_DELAYS_MS").ok();
        unsafe {
            std::env::set_var("CLANKERS_STDIO_RESTART_DELAYS_MS", value);
        }
        Self { _lock: lock, previous }
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

struct EnvVarGuard {
    _lock: MutexGuard<'static, ()>,
    previous: Vec<(&'static str, Option<String>)>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        Self::set_many(&[(key, value)])
    }

    fn set_many(entries: &[(&'static str, &str)]) -> Self {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let lock = LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut previous = Vec::with_capacity(entries.len());
        for (key, value) in entries {
            previous.push((*key, std::env::var(key).ok()));
            unsafe {
                std::env::set_var(key, value);
            }
        }
        Self { _lock: lock, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        for (key, previous) in self.previous.iter().rev() {
            if let Some(previous) = previous {
                unsafe {
                    std::env::set_var(key, previous);
                }
            } else {
                unsafe {
                    std::env::remove_var(key);
                }
            }
        }
    }
}

fn helper_binary() -> &'static PathBuf {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(&HELPER_SOURCE, &mut hasher);
        let base =
            std::env::temp_dir().join(format!("clankers-stdio-helper-{:016x}", std::hash::Hasher::finish(&hasher)));
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

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
            continue;
        }
        std::fs::copy(&src_path, &dst_path).unwrap();
        let permissions = std::fs::metadata(&src_path).unwrap().permissions();
        std::fs::set_permissions(&dst_path, permissions).unwrap();
    }
}

fn copy_reference_stdio_fixture(dst_root: &Path) -> PathBuf {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/plugins/clankers-stdio-echo");
    let dst = dst_root.join("clankers-stdio-echo");
    copy_dir_recursive(&source, &dst);
    dst
}

fn copy_extism_test_plugin_fixture(dst_root: &Path) -> PathBuf {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins/clankers-test-plugin");
    let dst = dst_root.join("clankers-test-plugin");
    copy_dir_recursive(&source, &dst);
    dst
}

pub(crate) fn write_stdio_plugin_manifest(
    dir: &Path,
    name: &str,
    behavior: &str,
    expected_mode: &str,
    state_file: Option<&Path>,
    shutdown_file: Option<&Path>,
) {
    write_stdio_plugin_manifest_with_policy(
        dir,
        name,
        behavior,
        expected_mode,
        state_file,
        shutdown_file,
        "plugin-dir",
        &["ui"],
        &[],
        "inherit",
    );
}

pub(crate) fn write_stdio_plugin_manifest_with_policy(
    dir: &Path,
    name: &str,
    behavior: &str,
    expected_mode: &str,
    state_file: Option<&Path>,
    shutdown_file: Option<&Path>,
    working_dir: &str,
    permissions: &[&str],
    env_allowlist: &[&str],
    sandbox: &str,
) {
    write_stdio_plugin_manifest_with_restricted_policy(
        dir,
        name,
        behavior,
        expected_mode,
        state_file,
        shutdown_file,
        working_dir,
        permissions,
        env_allowlist,
        sandbox,
        &[],
        false,
    );
}

pub(crate) fn write_stdio_plugin_manifest_with_restricted_policy(
    dir: &Path,
    name: &str,
    behavior: &str,
    expected_mode: &str,
    state_file: Option<&Path>,
    shutdown_file: Option<&Path>,
    working_dir: &str,
    permissions: &[&str],
    env_allowlist: &[&str],
    sandbox: &str,
    writable_roots: &[&str],
    allow_network: bool,
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
            "permissions": permissions,
            "stdio": {
                "command": helper.display().to_string(),
                "args": args,
                "working_dir": working_dir,
                "env_allowlist": env_allowlist,
                "sandbox": sandbox,
                "writable_roots": writable_roots,
                "allow_network": allow_network
            }
        }))
        .unwrap(),
    )
    .unwrap();
}

pub(crate) async fn wait_for_plugin_state(
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

pub(crate) async fn wait_for_live_tool(
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

async fn wait_for_stdio_host_events(
    manager: &Arc<Mutex<crate::plugin::PluginManager>>,
    timeout: Duration,
    predicate: impl Fn(&[crate::plugin::StdioHostEvent]) -> bool,
) -> Vec<crate::plugin::StdioHostEvent> {
    let deadline = tokio::time::Instant::now() + timeout;
    let mut collected = Vec::new();
    loop {
        let mut drained = crate::plugin::drain_stdio_host_events(manager);
        if !drained.is_empty() {
            collected.append(&mut drained);
            if predicate(&collected) {
                return collected;
            }
        }
        assert!(tokio::time::Instant::now() < deadline, "timed out waiting for stdio host events: {collected:?}");
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

fn tool_result_text(result: &crate::tools::ToolResult) -> String {
    match &result.content[0] {
        crate::tools::ToolResultContent::Text { text } => text.clone(),
        other => panic!("expected text content, got {:?}", other),
    }
}

async fn execute_plugin_tool(
    manager: &Arc<Mutex<crate::plugin::PluginManager>>,
    tool_name: &str,
    input: serde_json::Value,
) -> crate::tools::ToolResult {
    let tool = crate::modes::common::build_plugin_tools(&[], manager, None)
        .into_iter()
        .find(|tool| tool.definition().name == tool_name)
        .unwrap_or_else(|| panic!("tool '{tool_name}' not found"));
    let ctx = ToolContext::new(format!("call-{tool_name}"), CancellationToken::new(), None);
    tool.execute(&ctx, input).await
}

pub(crate) fn init_manager_with_restart_delays(
    dir: &Path,
    mode: PluginRuntimeMode,
    delays_ms: &str,
) -> Arc<Mutex<crate::plugin::PluginManager>> {
    let _guard = RestartDelayGuard::set(delays_ms);
    crate::modes::common::init_plugin_manager_for_mode(dir, None, &[], mode, dir)
}

#[tokio::test]
async fn standalone_stdio_plugin_launches_and_registers_live_tools() {
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(dir.path(), "stdio-standalone", "ready_register", "standalone", None, None);

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

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
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(dir.path(), "stdio-daemon", "ready_register", "daemon", None, None);

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Daemon, "5,10,15,20,25");

    let state = wait_for_plugin_state(&manager, "stdio-daemon", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    assert_eq!(state, PluginState::Active);

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn invalid_stdio_handshake_enters_error_state() {
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(dir.path(), "stdio-bad-handshake", "bad_handshake", "", None, None);

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    let state = wait_for_plugin_state(&manager, "stdio-bad-handshake", Duration::from_secs(20), |state| {
        matches!(state, PluginState::Error(_))
    })
    .await;
    assert!(matches!(state, PluginState::Error(message) if message.contains("ready before hello")));
}

#[tokio::test]
async fn active_stdio_crash_enters_backoff_then_restarts() {
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

    let manager =
        init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "500,1000,1500,2000,2500");

    let seen_states =
        sample_plugin_states(&manager, "stdio-restart", Duration::from_secs(2), Duration::from_millis(5)).await;
    assert!(
        seen_states
            .iter()
            .any(|state| matches!(state, PluginState::Backoff(message) if message.contains("intentional crash"))),
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

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

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
async fn shutdown_kills_unresponsive_plugin_after_grace_period() {
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-stuck-shutdown",
        "ignore_shutdown_forever",
        "standalone",
        None,
        None,
    );

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");
    let _grace = EnvVarGuard::set("CLANKERS_STDIO_SHUTDOWN_GRACE_MS", "100");

    wait_for_plugin_state(&manager, "stdio-stuck-shutdown", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_live_tool(&manager, "stdio-stuck-shutdown", "stdio_stuck_shutdown_tool", Duration::from_secs(2)).await;

    let start = std::time::Instant::now();
    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
    assert!(start.elapsed() < Duration::from_secs(2), "shutdown should not hang indefinitely");

    let state = wait_for_plugin_state(&manager, "stdio-stuck-shutdown", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Loaded)
    })
    .await;
    assert_eq!(state, PluginState::Loaded);
}

#[tokio::test]
async fn manual_disable_stops_stdio_plugin_without_scheduling_restart() {
    let dir = tempdir().unwrap();
    let counter = dir.path().join("disable-count.txt");
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-disable-no-restart",
        "count_launches_and_wait_shutdown",
        "standalone",
        Some(&counter),
        None,
    );

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");
    wait_for_plugin_state(&manager, "stdio-disable-no-restart", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;

    {
        let mut guard = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.disable("stdio-disable-no-restart").unwrap();
    }
    wait_for_plugin_state(&manager, "stdio-disable-no-restart", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Disabled)
    })
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(std::fs::read_to_string(&counter).unwrap().trim(), "1");
}

#[tokio::test]
async fn host_shutdown_stops_stdio_plugin_without_scheduling_restart() {
    let dir = tempdir().unwrap();
    let counter = dir.path().join("shutdown-count.txt");
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-shutdown-no-restart",
        "count_launches_and_wait_shutdown",
        "standalone",
        Some(&counter),
        None,
    );

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");
    wait_for_plugin_state(&manager, "stdio-shutdown-no-restart", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
    wait_for_plugin_state(&manager, "stdio-shutdown-no-restart", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Loaded)
    })
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(std::fs::read_to_string(&counter).unwrap().trim(), "1");
}

#[tokio::test]
async fn disable_then_enable_stdio_plugin_restarts_runtime() {
    let dir = tempdir().unwrap();
    let shutdown_marker = dir.path().join("shutdown-enable.txt");
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-enable",
        "ready_then_wait_shutdown",
        "standalone",
        None,
        Some(&shutdown_marker),
    );

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    wait_for_plugin_state(&manager, "stdio-enable", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_live_tool(&manager, "stdio-enable", "stdio_enable_tool", Duration::from_secs(2)).await;

    {
        let mut guard = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.disable("stdio-enable").unwrap();
    }
    wait_for_plugin_state(&manager, "stdio-enable", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Disabled)
    })
    .await;

    crate::plugin::enable_plugin(&manager, "stdio-enable").unwrap();
    wait_for_plugin_state(&manager, "stdio-enable", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_live_tool(&manager, "stdio-enable", "stdio_enable_tool", Duration::from_secs(2)).await;

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn reload_recovers_stdio_plugin_after_crash_loop_error() {
    let dir = tempdir().unwrap();
    let counter = dir.path().join("reload-count.txt");
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-reload-recovery",
        "bad_handshake_until_sixth_launch_then_register",
        "standalone",
        Some(&counter),
        None,
    );

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    let state = wait_for_plugin_state(&manager, "stdio-reload-recovery", Duration::from_secs(20), |state| {
        matches!(state, PluginState::Error(_))
    })
    .await;
    assert!(matches!(state, PluginState::Error(message) if message.contains("ready before hello")));

    crate::plugin::reload_plugin(&manager, "stdio-reload-recovery").unwrap();
    wait_for_plugin_state(&manager, "stdio-reload-recovery", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_live_tool(&manager, "stdio-reload-recovery", "stdio_reload_recovery_tool", Duration::from_secs(2)).await;
    assert_eq!(std::fs::read_to_string(&counter).unwrap().trim(), "6");

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn successful_ready_resets_consecutive_failure_counter() {
    let dir = tempdir().unwrap();
    let counter = dir.path().join("reset-count.txt");
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-reset-counter",
        "four_bad_then_ready_then_bad_then_register",
        "standalone",
        Some(&counter),
        None,
    );

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    let state = wait_for_plugin_state(&manager, "stdio-reset-counter", Duration::from_secs(5), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    assert_eq!(state, PluginState::Active);
    wait_for_live_tool(&manager, "stdio-reset-counter", "stdio_reset_counter_tool", Duration::from_secs(2)).await;
    assert_eq!(std::fs::read_to_string(&counter).unwrap().trim(), "7");

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn inherit_mode_launch_filters_env_and_uses_project_root_cwd() {
    let dir = tempdir().unwrap();
    let snapshot = dir.path().join("launch-snapshot.txt");
    write_stdio_plugin_manifest_with_policy(
        dir.path(),
        "stdio-inherit-env",
        "record_env_and_cwd",
        "standalone",
        Some(&snapshot),
        None,
        "project-root",
        &["ui"],
        &["GITHUB_TOKEN"],
        "inherit",
    );

    let _guard = EnvVarGuard::set_many(&[("GITHUB_TOKEN", "gh-secret"), ("SHOULD_NOT_LEAK", "still-hidden")]);
    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    wait_for_plugin_state(&manager, "stdio-inherit-env", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_live_tool(&manager, "stdio-inherit-env", "stdio_inherit_env_tool", Duration::from_secs(2)).await;

    let snapshot = std::fs::read_to_string(&snapshot).unwrap();
    assert!(snapshot.contains(&format!("cwd={}", dir.path().display())), "snapshot: {snapshot}");
    assert!(snapshot.contains("GITHUB_TOKEN=gh-secret"), "snapshot: {snapshot}");
    assert!(snapshot.contains("FASTMAIL_TOKEN=<unset>"), "snapshot: {snapshot}");
    assert!(snapshot.contains("SHOULD_NOT_LEAK=<unset>"), "snapshot: {snapshot}");
    assert!(snapshot.contains("PATH=<unset>"), "snapshot: {snapshot}");

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn inherit_mode_launch_uses_plugin_dir_when_requested() {
    let dir = tempdir().unwrap();
    let plugin_dir = dir.path().join("stdio-plugin-dir");
    let snapshot = plugin_dir.join("launch-snapshot.txt");
    write_stdio_plugin_manifest_with_policy(
        dir.path(),
        "stdio-plugin-dir",
        "record_env_and_cwd",
        "standalone",
        Some(&snapshot),
        None,
        "plugin-dir",
        &["ui"],
        &["GITHUB_TOKEN"],
        "inherit",
    );

    let _guard = EnvVarGuard::set_many(&[("GITHUB_TOKEN", "gh-plugin-dir"), ("SHOULD_NOT_LEAK", "still-hidden")]);
    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    wait_for_plugin_state(&manager, "stdio-plugin-dir", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_live_tool(&manager, "stdio-plugin-dir", "stdio_plugin_dir_tool", Duration::from_secs(2)).await;

    let snapshot = std::fs::read_to_string(&snapshot).unwrap();
    assert!(snapshot.contains(&format!("cwd={}", plugin_dir.display())), "snapshot: {snapshot}");
    assert!(snapshot.contains("GITHUB_TOKEN=gh-plugin-dir"), "snapshot: {snapshot}");
    assert!(snapshot.contains("SHOULD_NOT_LEAK=<unset>"), "snapshot: {snapshot}");
    assert!(snapshot.contains("PATH=<unset>"), "snapshot: {snapshot}");

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn restricted_writable_roots_outside_project_root_block_startup() {
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest_with_restricted_policy(
        dir.path(),
        "stdio-invalid-restricted-root",
        "record_env_and_cwd",
        "standalone",
        None,
        None,
        "plugin-dir",
        &["ui"],
        &[],
        "restricted",
        &["../escape"],
        false,
    );

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");
    let state = wait_for_plugin_state(&manager, "stdio-invalid-restricted-root", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Error(_))
    })
    .await;
    assert!(matches!(state, PluginState::Error(message) if message.contains("project root")));
}

#[tokio::test]
async fn missing_allowlisted_environment_variable_blocks_startup() {
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest_with_policy(
        dir.path(),
        "stdio-missing-env",
        "record_env_and_cwd",
        "standalone",
        None,
        None,
        "plugin-dir",
        &["ui"],
        &["FASTMAIL_TOKEN"],
        "inherit",
    );

    let _guard = EnvVarGuard::set("SHOULD_NOT_LEAK", "still-hidden");
    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    let state = wait_for_plugin_state(&manager, "stdio-missing-env", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Error(_))
    })
    .await;
    assert!(matches!(state, PluginState::Error(message) if message.contains("FASTMAIL_TOKEN")));
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn restricted_sandbox_bounds_writes_and_creates_state_dir() {
    let dir = tempdir().unwrap();
    let plugin_name = "stdio-restricted-write";
    let allowed_path = dir.path().join("build/output/allowed.txt");
    let denied_path = dir.path().join("denied.txt");
    let state_write_path = dir.path().join("plugin-state").join(plugin_name).join("state.txt");
    write_stdio_plugin_manifest_with_restricted_policy(
        dir.path(),
        plugin_name,
        "invoke_probe_sandbox",
        "standalone",
        None,
        None,
        "plugin-dir",
        &["ui"],
        &[
            "CLANKERS_TEST_ALLOWED_WRITE_PATH",
            "CLANKERS_TEST_DENIED_WRITE_PATH",
            "CLANKERS_TEST_STATE_WRITE_PATH",
        ],
        "restricted",
        &["build/output"],
        false,
    );

    let _guard = EnvVarGuard::set_many(&[
        ("CLANKERS_TEST_ALLOWED_WRITE_PATH", &allowed_path.display().to_string()),
        ("CLANKERS_TEST_DENIED_WRITE_PATH", &denied_path.display().to_string()),
        ("CLANKERS_TEST_STATE_WRITE_PATH", &state_write_path.display().to_string()),
    ]);
    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    wait_for_plugin_state(&manager, plugin_name, Duration::from_secs(2), |state| matches!(state, PluginState::Active))
        .await;
    wait_for_live_tool(&manager, plugin_name, "stdio_restricted_write_tool", Duration::from_secs(2)).await;

    let result = execute_plugin_tool(&manager, "stdio_restricted_write_tool", serde_json::json!({})).await;
    assert!(!result.is_error, "sandbox probe tool should succeed: {:?}", result);
    let text = tool_result_text(&result);
    assert!(text.contains("state_write=ok"), "result: {text}");
    assert!(text.contains("allowed_write=ok"), "result: {text}");
    assert!(text.contains("denied_write=err:"), "result: {text}");
    assert!(state_write_path.exists(), "expected dedicated state dir write at {}", state_write_path.display());
    assert!(allowed_path.exists(), "expected allowed restricted write at {}", allowed_path.display());
    assert!(!denied_path.exists(), "unexpected write outside restricted roots at {}", denied_path.display());

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn restricted_sandbox_denies_network_without_allow_network() {
    let dir = tempdir().unwrap();
    let plugin_name = "stdio-restricted-no-net";
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    write_stdio_plugin_manifest_with_restricted_policy(
        dir.path(),
        plugin_name,
        "invoke_probe_sandbox",
        "standalone",
        None,
        None,
        "plugin-dir",
        &["ui", "net"],
        &["CLANKERS_TEST_NETWORK_ADDR"],
        "restricted",
        &[],
        false,
    );

    let addr_string = addr.to_string();
    let _guard = EnvVarGuard::set("CLANKERS_TEST_NETWORK_ADDR", &addr_string);
    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    wait_for_plugin_state(&manager, plugin_name, Duration::from_secs(2), |state| matches!(state, PluginState::Active))
        .await;
    wait_for_live_tool(&manager, plugin_name, "stdio_restricted_no_net_tool", Duration::from_secs(2)).await;

    let result = execute_plugin_tool(&manager, "stdio_restricted_no_net_tool", serde_json::json!({})).await;
    assert!(!result.is_error, "sandbox probe tool should succeed: {:?}", result);
    let text = tool_result_text(&result);
    assert!(text.contains("network=err:"), "result: {text}");
    assert!(
        text.contains("Operation not permitted") || text.contains("Permission denied"),
        "expected denied network error, got: {text}"
    );
    assert!(tokio::time::timeout(Duration::from_millis(200), listener.accept()).await.is_err());

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn restricted_sandbox_allows_network_when_permission_and_policy_allow() {
    let dir = tempdir().unwrap();
    let plugin_name = "stdio-restricted-net-ok";
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    write_stdio_plugin_manifest_with_restricted_policy(
        dir.path(),
        plugin_name,
        "invoke_probe_sandbox",
        "standalone",
        None,
        None,
        "plugin-dir",
        &["ui", "net"],
        &["CLANKERS_TEST_NETWORK_ADDR"],
        "restricted",
        &[],
        true,
    );

    let addr_string = addr.to_string();
    let _guard = EnvVarGuard::set("CLANKERS_TEST_NETWORK_ADDR", &addr_string);
    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    wait_for_plugin_state(&manager, plugin_name, Duration::from_secs(2), |state| matches!(state, PluginState::Active))
        .await;
    wait_for_live_tool(&manager, plugin_name, "stdio_restricted_net_ok_tool", Duration::from_secs(2)).await;

    let accept = tokio::spawn(async move { listener.accept().await });
    let result = execute_plugin_tool(&manager, "stdio_restricted_net_ok_tool", serde_json::json!({})).await;
    assert!(!result.is_error, "sandbox probe tool should succeed: {:?}", result);
    let text = tool_result_text(&result);
    assert!(text.contains("network=ok"), "result: {text}");
    tokio::time::timeout(Duration::from_secs(2), accept).await.unwrap().unwrap().unwrap();

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn restricted_sandbox_mode_fails_closed_when_backend_is_unavailable() {
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest_with_policy(
        dir.path(),
        "stdio-restricted",
        "record_env_and_cwd",
        "standalone",
        None,
        None,
        "plugin-dir",
        &["ui"],
        &[],
        "restricted",
    );

    let _guard = EnvVarGuard::set("CLANKERS_STDIO_FORCE_RESTRICTED_UNAVAILABLE", "1");
    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");
    let state = wait_for_plugin_state(&manager, "stdio-restricted", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Error(_))
    })
    .await;
    assert!(matches!(state, PluginState::Error(message) if message.contains("restricted sandbox mode is unavailable")));
}

#[tokio::test]
async fn stderr_is_included_in_launch_diagnostics() {
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(dir.path(), "stdio-stderr", "stderr_bad_handshake", "", None, None);

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    let state = wait_for_plugin_state(&manager, "stdio-stderr", Duration::from_secs(20), |state| {
        matches!(state, PluginState::Error(_))
    })
    .await;
    assert!(matches!(state, PluginState::Error(message) if message.contains("helper stderr launch diagnostic")));
}

#[tokio::test]
async fn additive_register_tools_calls_keep_earlier_stdio_tools_active() {
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-additive-tools",
        "register_tools_additive",
        "standalone",
        None,
        None,
    );

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    wait_for_plugin_state(&manager, "stdio-additive-tools", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_live_tool(&manager, "stdio-additive-tools", "stdio_additive_tools_tool", Duration::from_secs(2)).await;
    wait_for_live_tool(&manager, "stdio-additive-tools", "stdio_additive_tools_tool_extra", Duration::from_secs(2))
        .await;

    let tool_names: Vec<String> = crate::modes::common::build_plugin_tools(&[], &manager, None)
        .into_iter()
        .map(|tool| tool.definition().name.clone())
        .collect();
    assert!(tool_names.contains(&"stdio_additive_tools_tool".to_string()));
    assert!(tool_names.contains(&"stdio_additive_tools_tool_extra".to_string()));

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn mixed_runtime_host_preserves_extism_behavior_and_stdio_visibility() {
    let dir = tempdir().unwrap();
    copy_extism_test_plugin_fixture(dir.path());
    copy_reference_stdio_fixture(dir.path());
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-mixed-event-ui",
        "event_tool_call_ui_display",
        "standalone",
        None,
        None,
    );

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    wait_for_plugin_state(&manager, "clankers-test-plugin", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_plugin_state(&manager, "clankers-stdio-echo", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_plugin_state(&manager, "stdio-mixed-event-ui", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_live_tool(&manager, "clankers-stdio-echo", "stdio_echo_fixture", Duration::from_secs(2)).await;
    wait_for_live_tool(&manager, "stdio-mixed-event-ui", "stdio_mixed_event_ui_tool", Duration::from_secs(2)).await;

    let summaries = crate::plugin::build_protocol_plugin_summaries(&manager);
    let extism = summaries.iter().find(|summary| summary.name == "clankers-test-plugin").unwrap();
    assert_eq!(extism.kind.as_deref(), Some("extism"));
    assert!(extism.tools.contains(&"test_echo".to_string()));
    let stdio = summaries.iter().find(|summary| summary.name == "clankers-stdio-echo").unwrap();
    assert_eq!(stdio.kind.as_deref(), Some("stdio"));
    assert!(stdio.tools.contains(&"stdio_echo_fixture".to_string()));

    let tools = crate::modes::common::build_plugin_tools(&[], &manager, None);
    let names: Vec<String> = tools.iter().map(|tool| tool.definition().name.clone()).collect();
    assert!(names.contains(&"test_echo".to_string()));
    assert!(names.contains(&"stdio_echo_fixture".to_string()));
    assert!(names.contains(&"stdio_mixed_event_ui_tool".to_string()));

    let extism_tool = tools.iter().find(|tool| tool.definition().name == "test_echo").unwrap().clone();
    let ctx = ToolContext::new("call-mixed-extism".to_string(), CancellationToken::new(), None);
    let result = extism_tool.execute(&ctx, serde_json::json!({"text": "mixed hello"})).await;
    assert!(!result.is_error, "extism tool should still work: {:?}", result);
    assert_eq!(tool_result_text(&result), "mixed hello");

    let stdio_tool = crate::modes::common::build_plugin_tools(&[], &manager, None)
        .into_iter()
        .find(|tool| tool.definition().name == "stdio_echo_fixture")
        .unwrap();
    let ctx = ToolContext::new("call-mixed-stdio".to_string(), CancellationToken::new(), None);
    let result = stdio_tool.execute(&ctx, serde_json::json!({"mode": "echo", "message": "mixed hi"})).await;
    assert!(!result.is_error, "stdio fixture should still work: {:?}", result);
    assert_eq!(tool_result_text(&result), "fixture:mixed hi");

    let host = crate::plugin::PluginHostFacade::new(Arc::clone(&manager));
    let extism_subscribers: Vec<String> =
        host.event_subscribers("tool_call").into_iter().map(|info| info.name).collect();
    let stdio_subscribers: Vec<String> =
        host.stdio_event_subscribers("tool_call").into_iter().map(|info| info.name).collect();
    assert!(extism_subscribers.contains(&"clankers-test-plugin".to_string()));
    assert!(stdio_subscribers.contains(&"stdio-mixed-event-ui".to_string()));

    let extism_result = host
        .call_plugin("clankers-test-plugin", "on_event", r#"{"event":"tool_call","data":{"tool":"bash"}}"#)
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&extism_result).unwrap();
    assert_eq!(parsed["handled"], true);
    assert_eq!(parsed["message"], "Observed tool call: bash");

    let immediate = crate::modes::plugin_dispatch::dispatch_event_to_plugins(&manager, &AgentEvent::ToolCall {
        tool_name: "bash".to_string(),
        call_id: "call-mixed-event".to_string(),
        input: serde_json::json!({"command": "echo hi"}),
    });
    let mut messages = immediate.messages;
    let mut ui_actions = immediate.ui_actions;
    if messages.is_empty() || ui_actions.len() < 3 {
        let events = wait_for_stdio_host_events(&manager, Duration::from_secs(2), |events| events.len() >= 4).await;
        for event in events {
            match event {
                crate::plugin::StdioHostEvent::Display { plugin, message } => messages.push((plugin, message)),
                crate::plugin::StdioHostEvent::Ui(action) => ui_actions.push(action),
            }
        }
    }
    assert!(
        messages
            .iter()
            .any(|(plugin, message)| plugin == "stdio-mixed-event-ui" && message.contains("tool_call for bash")),
        "expected stdio event visibility, got: {messages:?}"
    );
    assert!(ui_actions.len() >= 3, "expected stdio UI actions, got: {ui_actions:?}");

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn mixed_runtime_rejects_cross_kind_tool_name_collision_deterministically() {
    let dir = tempdir().unwrap();
    copy_extism_test_plugin_fixture(dir.path());
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-extism-collision",
        "ready_register_with_extism_collision",
        "standalone",
        None,
        None,
    );

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    wait_for_plugin_state(&manager, "clankers-test-plugin", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_plugin_state(&manager, "stdio-extism-collision", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_live_tool(&manager, "stdio-extism-collision", "stdio_extism_collision_tool_extra", Duration::from_secs(2))
        .await;

    let summaries = crate::plugin::build_protocol_plugin_summaries(&manager);
    let stdio = summaries.iter().find(|summary| summary.name == "stdio-extism-collision").unwrap();
    assert!(stdio.tools.contains(&"stdio_extism_collision_tool_extra".to_string()));
    assert!(!stdio.tools.contains(&"test_echo".to_string()), "cross-kind collision should be rejected");

    let tool_names: Vec<String> = crate::modes::common::build_plugin_tools(&[], &manager, None)
        .into_iter()
        .map(|tool| tool.definition().name.clone())
        .collect();
    assert!(tool_names.contains(&"test_echo".to_string()));
    assert!(tool_names.contains(&"stdio_extism_collision_tool_extra".to_string()));
    assert_eq!(tool_names.iter().filter(|name| name.as_str() == "test_echo").count(), 1);

    let extism_tool = crate::modes::common::build_plugin_tools(&[], &manager, None)
        .into_iter()
        .find(|tool| tool.definition().name == "test_echo")
        .unwrap();
    let ctx = ToolContext::new("call-cross-kind-collision".to_string(), CancellationToken::new(), None);
    let result = extism_tool.execute(&ctx, serde_json::json!({"text": "still extism"})).await;
    assert!(!result.is_error, "extism tool should keep ownership: {:?}", result);
    assert_eq!(tool_result_text(&result), "still extism");

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn reference_stdio_fixture_exercises_invoke_cancel_and_shutdown_end_to_end() {
    let dir = tempdir().unwrap();
    copy_reference_stdio_fixture(dir.path());

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    wait_for_plugin_state(&manager, "clankers-stdio-echo", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_live_tool(&manager, "clankers-stdio-echo", "stdio_echo_fixture", Duration::from_secs(2)).await;

    let tools = crate::modes::common::build_plugin_tools(&[], &manager, None);
    let tool = tools.into_iter().find(|tool| tool.definition().name == "stdio_echo_fixture").unwrap();
    let ctx = ToolContext::new("call-stdio-echo-fixture".to_string(), CancellationToken::new(), None);
    let result = tool.execute(&ctx, serde_json::json!({"mode": "echo", "message": "hi"})).await;
    assert!(!result.is_error, "fixture echo should succeed: {:?}", result);
    assert_eq!(tool_result_text(&result), "fixture:hi");

    let tools = crate::modes::common::build_plugin_tools(&[], &manager, None);
    let tool = tools.into_iter().find(|tool| tool.definition().name == "stdio_echo_fixture").unwrap();
    let cancel = CancellationToken::new();
    let ctx = ToolContext::new("call-stdio-echo-fixture-cancel".to_string(), cancel.clone(), None);
    let task = tokio::spawn(async move { tool.execute(&ctx, serde_json::json!({"mode": "wait_for_cancel"})).await });
    tokio::time::sleep(Duration::from_millis(50)).await;
    cancel.cancel();
    let result = task.await.unwrap();
    assert!(result.is_error, "fixture cancel should surface error: {:?}", result);
    assert!(tool_result_text(&result).contains("cancelled"));

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
    wait_for_plugin_state(&manager, "clankers-stdio-echo", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Loaded)
    })
    .await;
}

#[tokio::test]
async fn live_stdio_tool_builds_and_executes_real_tool_adapter() {
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(dir.path(), "stdio-invoke", "invoke_success", "standalone", None, None);

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    wait_for_plugin_state(&manager, "stdio-invoke", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_live_tool(&manager, "stdio-invoke", "stdio_invoke_tool", Duration::from_secs(2)).await;

    let tools = crate::modes::common::build_plugin_tools(&[], &manager, None);
    let tool = tools.into_iter().find(|tool| tool.definition().name == "stdio_invoke_tool").unwrap();
    assert_eq!(tool.source(), "stdio-invoke");

    let tiered_names: Vec<String> =
        crate::modes::common::build_all_tiered_tools(&crate::modes::common::ToolEnv::default(), Some(&manager))
            .into_iter()
            .map(|(_, tool)| tool.definition().name.clone())
            .collect();
    assert!(tiered_names.contains(&"stdio_invoke_tool".to_string()));

    let (event_tx, mut event_rx) = broadcast::channel(16);
    let ctx = ToolContext::new("call-stdio-invoke".to_string(), CancellationToken::new(), Some(event_tx));
    let result = tool.execute(&ctx, serde_json::json!({"message": "hello"})).await;
    assert!(!result.is_error, "stdio invoke should succeed: {:?}", result);
    assert_eq!(tool_result_text(&result), "helper result");

    let mut saw_progress = false;
    while let Ok(event) = event_rx.try_recv() {
        if let AgentEvent::ToolExecutionUpdate { partial, .. } = event {
            let text = tool_result_text(&partial);
            if text.contains("helper progress") {
                saw_progress = true;
            }
        }
    }
    assert!(saw_progress, "expected helper progress event");

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn colliding_stdio_tool_registration_rejects_builtin_name() {
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-collision",
        "ready_register_with_builtin_collision",
        "standalone",
        None,
        None,
    );

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    wait_for_plugin_state(&manager, "stdio-collision", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_live_tool(&manager, "stdio-collision", "stdio_collision_tool", Duration::from_secs(2)).await;

    let summaries = crate::plugin::build_protocol_plugin_summaries(&manager);
    let plugin = summaries.iter().find(|summary| summary.name == "stdio-collision").unwrap();
    assert!(plugin.tools.iter().any(|tool| tool == "stdio_collision_tool"));
    assert!(!plugin.tools.iter().any(|tool| tool == "read"), "builtin collision should be rejected");

    let tool_names: Vec<String> = crate::modes::common::build_plugin_tools(&[], &manager, None)
        .into_iter()
        .map(|tool| tool.definition().name.clone())
        .collect();
    assert!(tool_names.contains(&"stdio_collision_tool".to_string()));
    assert!(!tool_names.contains(&"read".to_string()));

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn cancelled_stdio_tool_call_sends_cancel_and_returns_cancelled_error() {
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(dir.path(), "stdio-cancel", "invoke_wait_for_cancel", "standalone", None, None);

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");
    let _cancel_timeout = EnvVarGuard::set("CLANKERS_STDIO_TOOL_CANCEL_TIMEOUT_MS", "100");

    wait_for_plugin_state(&manager, "stdio-cancel", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_live_tool(&manager, "stdio-cancel", "stdio_cancel_tool", Duration::from_secs(2)).await;

    let tools = crate::modes::common::build_plugin_tools(&[], &manager, None);
    let tool = tools.into_iter().find(|tool| tool.definition().name == "stdio_cancel_tool").unwrap();

    let cancel = CancellationToken::new();
    let ctx = ToolContext::new("call-stdio-cancel".to_string(), cancel.clone(), None);
    let task = tokio::spawn(async move { tool.execute(&ctx, serde_json::json!({})).await });
    tokio::time::sleep(Duration::from_millis(50)).await;
    cancel.cancel();

    let result = task.await.unwrap();
    assert!(result.is_error, "cancelled tool should surface error: {:?}", result);
    assert!(tool_result_text(&result).contains("cancelled"));

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn hung_stdio_tool_call_times_out() {
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(dir.path(), "stdio-timeout", "invoke_never_finishes", "standalone", None, None);

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");
    let _tool_timeout = EnvVarGuard::set("CLANKERS_STDIO_TOOL_TIMEOUT_MS", "100");

    wait_for_plugin_state(&manager, "stdio-timeout", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_live_tool(&manager, "stdio-timeout", "stdio_timeout_tool", Duration::from_secs(2)).await;

    let tools = crate::modes::common::build_plugin_tools(&[], &manager, None);
    let tool = tools.into_iter().find(|tool| tool.definition().name == "stdio_timeout_tool").unwrap();

    let ctx = ToolContext::new("call-stdio-timeout".to_string(), CancellationToken::new(), None);
    let result = tool.execute(&ctx, serde_json::json!({})).await;
    assert!(result.is_error, "hung tool should time out: {:?}", result);
    assert!(tool_result_text(&result).contains("timed out"));

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn restarted_stdio_plugin_must_resubscribe_before_receiving_events_again() {
    let dir = tempdir().unwrap();
    let counter = dir.path().join("restart-subscribe-count.txt");
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-resubscribe",
        "subscribe_then_crash_then_do_not_resubscribe",
        "standalone",
        Some(&counter),
        None,
    );

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    wait_for_plugin_state(&manager, "stdio-resubscribe", Duration::from_secs(5), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    assert_eq!(std::fs::read_to_string(&counter).unwrap().trim(), "2");

    let immediate = crate::modes::plugin_dispatch::dispatch_event_to_plugins(&manager, &AgentEvent::ToolCall {
        tool_name: "bash".to_string(),
        call_id: "call-stdio-resubscribe".to_string(),
        input: serde_json::json!({"command": "echo hi"}),
    });
    assert!(immediate.messages.is_empty(), "unexpected immediate messages: {:?}", immediate.messages);
    assert!(immediate.ui_actions.is_empty(), "unexpected immediate ui actions: {:?}", immediate.ui_actions);

    tokio::time::sleep(Duration::from_millis(150)).await;
    let drained = crate::plugin::drain_stdio_host_events(&manager);
    assert!(drained.is_empty(), "unexpected stdio host events after restart without resubscribe: {drained:?}");

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn subscribed_stdio_plugin_receives_tool_call_event_and_emits_ui_and_display() {
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(dir.path(), "stdio-event-ui", "event_tool_call_ui_display", "standalone", None, None);

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    wait_for_plugin_state(&manager, "stdio-event-ui", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;

    let immediate = crate::modes::plugin_dispatch::dispatch_event_to_plugins(&manager, &AgentEvent::ToolCall {
        tool_name: "bash".to_string(),
        call_id: "call-stdio-event".to_string(),
        input: serde_json::json!({"command": "echo hi"}),
    });

    let mut messages = immediate.messages;
    let mut ui_actions = immediate.ui_actions;
    if messages.is_empty() || ui_actions.len() < 3 {
        let events = wait_for_stdio_host_events(&manager, Duration::from_secs(2), |events| events.len() >= 4).await;
        for event in events {
            match event {
                crate::plugin::StdioHostEvent::Display { plugin, message } => messages.push((plugin, message)),
                crate::plugin::StdioHostEvent::Ui(action) => ui_actions.push(action),
            }
        }
    }

    assert!(
        messages
            .iter()
            .any(|(plugin, message)| plugin == "stdio-event-ui" && message.contains("tool_call for bash")),
        "expected stdio display message, got: {messages:?}"
    );
    assert!(
        ui_actions.iter().any(|action| matches!(
            action,
            crate::plugin::ui::PluginUiAction::SetStatus { plugin, text, color }
                if plugin == "stdio-event-ui" && text == "tool bash" && color.as_deref() == Some("green")
        )),
        "expected set_status action, got: {ui_actions:?}"
    );
    assert!(
        ui_actions.iter().any(|action| matches!(
            action,
            crate::plugin::ui::PluginUiAction::Notify { plugin, message, level }
                if plugin == "stdio-event-ui" && message == "note bash" && level == "info"
        )),
        "expected notify action, got: {ui_actions:?}"
    );
    assert!(
        ui_actions.iter().any(|action| matches!(
            action,
            crate::plugin::ui::PluginUiAction::SetWidget { plugin, widget: crate::plugin::ui::Widget::Text { content, .. } }
                if plugin == "stdio-event-ui" && content == "widget bash"
        )),
        "expected set_widget action, got: {ui_actions:?}"
    );

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn unsubscribed_stdio_plugin_does_not_receive_tool_call_event() {
    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(
        dir.path(),
        "stdio-event-unsubscribed",
        "event_unsubscribed_emit_if_called",
        "standalone",
        None,
        None,
    );

    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");

    wait_for_plugin_state(&manager, "stdio-event-unsubscribed", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;

    let immediate = crate::modes::plugin_dispatch::dispatch_event_to_plugins(&manager, &AgentEvent::ToolCall {
        tool_name: "bash".to_string(),
        call_id: "call-stdio-unsubscribed".to_string(),
        input: serde_json::json!({"command": "echo hi"}),
    });
    assert!(immediate.messages.is_empty(), "unexpected immediate messages: {:?}", immediate.messages);
    assert!(immediate.ui_actions.is_empty(), "unexpected immediate ui actions: {:?}", immediate.ui_actions);

    tokio::time::sleep(Duration::from_millis(150)).await;
    let drained = crate::plugin::drain_stdio_host_events(&manager);
    assert!(drained.is_empty(), "unexpected stdio host events: {drained:?}");

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn capability_gate_blocks_stdio_tool_calls_in_turn_loop() {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use clankers_ucan::Capability;
    use tokio::sync::mpsc;

    use crate::message::AgentMessage;
    use crate::message::Content;
    use crate::message::MessageId;
    use crate::message::UserMessage;
    use crate::provider::streaming::MessageMetadata;
    use crate::provider::streaming::StreamEvent;
    use crate::provider::streaming::Usage;

    struct ToolUseProvider {
        calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl crate::provider::Provider for ToolUseProvider {
        async fn complete(
            &self,
            _request: crate::provider::CompletionRequest,
            tx: mpsc::Sender<StreamEvent>,
        ) -> crate::provider::error::Result<()> {
            let usage = Usage {
                input_tokens: 10,
                output_tokens: 2,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            };
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                tx.send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: "msg-stdio-1".into(),
                        model: "test-model".into(),
                        role: "assistant".into(),
                    },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockStart {
                    index: 0,
                    content_block: Content::ToolUse {
                        id: "toolu-stdio-1".into(),
                        name: "stdio_capability_tool".into(),
                        input: serde_json::json!({}),
                    },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                tx.send(StreamEvent::MessageDelta {
                    stop_reason: Some("tool_use".into()),
                    usage,
                })
                .await
                .ok();
                tx.send(StreamEvent::MessageStop).await.ok();
            } else {
                tx.send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: "msg-stdio-2".into(),
                        model: "test-model".into(),
                        role: "assistant".into(),
                    },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockStart {
                    index: 0,
                    content_block: Content::Text { text: String::new() },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: crate::provider::streaming::ContentDelta::TextDelta { text: "done".into() },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                tx.send(StreamEvent::MessageDelta {
                    stop_reason: Some("end_turn".into()),
                    usage,
                })
                .await
                .ok();
                tx.send(StreamEvent::MessageStop).await.ok();
            }
            Ok(())
        }

        fn models(&self) -> &[crate::provider::Model] {
            &[]
        }

        fn name(&self) -> &str {
            "stdio-capability"
        }
    }

    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(dir.path(), "stdio-capability", "invoke_success", "standalone", None, None);
    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");
    wait_for_plugin_state(&manager, "stdio-capability", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_live_tool(&manager, "stdio-capability", "stdio_capability_tool", Duration::from_secs(2)).await;

    let tools = crate::modes::common::build_plugin_tools(&[], &manager, None);
    let tools: HashMap<String, Arc<dyn crate::tools::Tool>> =
        tools.into_iter().map(|tool| (tool.definition().name.clone(), tool)).collect();
    let mut messages = vec![AgentMessage::User(UserMessage {
        id: MessageId::new("user-stdio-capability"),
        content: vec![Content::Text { text: "hi".into() }],
        timestamp: chrono::Utc::now(),
    })];
    let config = crate::agent::turn::TurnConfig {
        model: "test-model".into(),
        system_prompt: "You are a test assistant.".into(),
        max_tokens: Some(100),
        temperature: None,
        thinking: None,
        max_turns: 2,
        output_truncation: clanker_loop::OutputTruncationConfig::default(),
        no_cache: true,
        cache_ttl: None,
    };
    let (event_tx, _event_rx) = broadcast::channel(64);
    let gate: Arc<dyn crate::agent::CapabilityGate> =
        Arc::new(crate::capability_gate::UcanCapabilityGate::new(vec![Capability::ToolUse {
            tool_pattern: "read,bash".to_string(),
        }]));

    crate::agent::turn::run_turn_loop(
        &ToolUseProvider {
            calls: AtomicUsize::new(0),
        },
        &tools,
        &mut messages,
        &config,
        &event_tx,
        CancellationToken::new(),
        None,
        None,
        None,
        "session-stdio-capability",
        None,
        Some(&gate),
        None,
    )
    .await
    .unwrap();

    let tool_result = messages
        .iter()
        .find_map(|message| match message {
            AgentMessage::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("tool result present");
    assert!(tool_result.is_error);
    match &tool_result.content[0] {
        Content::Text { text } => assert!(text.contains("🔒"), "expected lock error, got: {text}"),
        other => panic!("expected text tool result, got {:?}", other),
    }

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}

#[tokio::test]
async fn capability_gate_allows_stdio_tool_calls_in_turn_loop() {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use clankers_ucan::Capability;
    use tokio::sync::mpsc;

    use crate::message::AgentMessage;
    use crate::message::Content;
    use crate::message::MessageId;
    use crate::message::UserMessage;
    use crate::provider::streaming::MessageMetadata;
    use crate::provider::streaming::StreamEvent;
    use crate::provider::streaming::Usage;

    struct ToolUseProvider {
        calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl crate::provider::Provider for ToolUseProvider {
        async fn complete(
            &self,
            _request: crate::provider::CompletionRequest,
            tx: mpsc::Sender<StreamEvent>,
        ) -> crate::provider::error::Result<()> {
            let usage = Usage {
                input_tokens: 10,
                output_tokens: 2,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            };
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                tx.send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: "msg-stdio-allow-1".into(),
                        model: "test-model".into(),
                        role: "assistant".into(),
                    },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockStart {
                    index: 0,
                    content_block: Content::ToolUse {
                        id: "toolu-stdio-allow-1".into(),
                        name: "stdio_capability_tool".into(),
                        input: serde_json::json!({}),
                    },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                tx.send(StreamEvent::MessageDelta {
                    stop_reason: Some("tool_use".into()),
                    usage,
                })
                .await
                .ok();
                tx.send(StreamEvent::MessageStop).await.ok();
            } else {
                tx.send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: "msg-stdio-allow-2".into(),
                        model: "test-model".into(),
                        role: "assistant".into(),
                    },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockStart {
                    index: 0,
                    content_block: Content::Text { text: String::new() },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: crate::provider::streaming::ContentDelta::TextDelta { text: "done".into() },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                tx.send(StreamEvent::MessageDelta {
                    stop_reason: Some("end_turn".into()),
                    usage,
                })
                .await
                .ok();
                tx.send(StreamEvent::MessageStop).await.ok();
            }
            Ok(())
        }

        fn models(&self) -> &[crate::provider::Model] {
            &[]
        }

        fn name(&self) -> &str {
            "stdio-capability"
        }
    }

    let dir = tempdir().unwrap();
    write_stdio_plugin_manifest(dir.path(), "stdio-capability", "invoke_success", "standalone", None, None);
    let manager = init_manager_with_restart_delays(dir.path(), PluginRuntimeMode::Standalone, "5,10,15,20,25");
    wait_for_plugin_state(&manager, "stdio-capability", Duration::from_secs(2), |state| {
        matches!(state, PluginState::Active)
    })
    .await;
    wait_for_live_tool(&manager, "stdio-capability", "stdio_capability_tool", Duration::from_secs(2)).await;

    let tools = crate::modes::common::build_plugin_tools(&[], &manager, None);
    let tools: HashMap<String, Arc<dyn crate::tools::Tool>> =
        tools.into_iter().map(|tool| (tool.definition().name.clone(), tool)).collect();
    let mut messages = vec![AgentMessage::User(UserMessage {
        id: MessageId::new("user-stdio-capability-allow"),
        content: vec![Content::Text { text: "hi".into() }],
        timestamp: chrono::Utc::now(),
    })];
    let config = crate::agent::turn::TurnConfig {
        model: "test-model".into(),
        system_prompt: "You are a test assistant.".into(),
        max_tokens: Some(100),
        temperature: None,
        thinking: None,
        max_turns: 2,
        output_truncation: clanker_loop::OutputTruncationConfig::default(),
        no_cache: true,
        cache_ttl: None,
    };
    let (event_tx, _event_rx) = broadcast::channel(64);
    let gate: Arc<dyn crate::agent::CapabilityGate> =
        Arc::new(crate::capability_gate::UcanCapabilityGate::new(vec![Capability::ToolUse {
            tool_pattern: "stdio_capability_tool".to_string(),
        }]));

    crate::agent::turn::run_turn_loop(
        &ToolUseProvider {
            calls: AtomicUsize::new(0),
        },
        &tools,
        &mut messages,
        &config,
        &event_tx,
        CancellationToken::new(),
        None,
        None,
        None,
        "session-stdio-capability-allow",
        None,
        Some(&gate),
        None,
    )
    .await
    .unwrap();

    let tool_result = messages
        .iter()
        .find_map(|message| match message {
            AgentMessage::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("tool result present");
    assert!(!tool_result.is_error);
    match &tool_result.content[0] {
        Content::Text { text } => assert_eq!(text, "helper result"),
        other => panic!("expected text tool result, got {:?}", other),
    }

    crate::plugin::shutdown_plugin_runtime(&manager, "test shutdown").await;
}
