#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
serde_json = "1"
---

use std::env;
use std::fs;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde_json::Value;
use serde_json::json;

const ERROR_EXIT: u8 = 1;
const SENTINEL_START: &str = "CLANKERS_BG_DOGFOOD_START";
const SENTINEL_DONE: &str = "CLANKERS_BG_DOGFOOD_DONE";
const SESSION_PREFIX: &str = "clankers-bg-dogfood";
const PROCESS_SLEEP_SECONDS: u64 = 12;

fn main() -> ExitCode {
    match run() {
        Ok(receipt_path) => {
            println!("background process TUI dogfood receipt written to {}", receipt_path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("background process TUI dogfood failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    ensure_tmux_available()?;
    let binary = clankers_binary()?;
    let output_dir = output_dir()?;
    fs::create_dir_all(&output_dir).map_err(|error| format!("failed to create {}: {error}", output_dir.display()))?;

    let (port, _server_thread, request_count) = start_provider_stub()?;
    let session = format!("{SESSION_PREFIX}-{}", std::process::id());
    let mut guard = TmuxSessionGuard::new(session.clone());
    launch_clankers_tmux(&session, &binary, port)?;
    guard.mark_started();

    wait_for_screen(&session, |screen| screen.contains("NORMAL"), Duration::from_secs(15), "initial NORMAL mode")?;
    slash(&session, "/layout focused")?;
    wait_for_screen(&session, |screen| screen.contains("Layout: focused"), Duration::from_secs(8), "focused layout")?;
    slash(&session, "/layout toggle bg")?;
    let before = wait_for_screen(
        &session,
        |screen| screen.contains("Spawned/BG (0 active)"),
        Duration::from_secs(12),
        "empty background panel after /layout toggle bg",
    )?;

    send_key(&session, "Escape")?;
    sleep_ms(100);
    send_key(&session, "i")?;
    sleep_ms(100);
    send_literal(
        &session,
        "Start the background process dogfood now. Use the process tool exactly as requested by the test harness; do not use any other tool. Then summarize briefly.",
    )?;
    sleep_ms(200);
    send_key(&session, "Enter")?;

    wait_for_screen(
        &session,
        |screen| active_count(screen).unwrap_or(0) > 0,
        Duration::from_secs(30),
        "background panel with active process",
    )?;
    sleep_ms(750);
    let active = capture_screen(&session)?;

    fs::write(output_dir.join("screen-before-process.txt"), &before)
        .map_err(|error| format!("failed to write pre-process screen: {error}"))?;
    fs::write(output_dir.join("screen-active-process.txt"), &active)
        .map_err(|error| format!("failed to write active-process screen: {error}"))?;

    let active_processes = active_count(&active).unwrap_or(0);
    let command_visible = [SENTINEL_START, "sleep 12", "bash -lc"].iter().any(|needle| active.contains(needle));
    let layout_toggle_bg_visible = before.contains("Spawned/BG (0 active)");

    if active_processes == 0 {
        return Err("background panel did not report an active process".to_string());
    }
    if !command_visible {
        return Err("background process command/sentinel was not visible in the panel".to_string());
    }
    if !layout_toggle_bg_visible {
        return Err("/layout toggle bg did not render the background panel before process start".to_string());
    }

    guard.close();
    wait_for_sentinel_process_exit(Duration::from_secs(PROCESS_SLEEP_SECONDS + 8))?;

    let receipt = json!({
        "schema": "clankers.bg_process_tui_dogfood.receipt.v1",
        "result": "pass",
        "session": session,
        "provider_requests": request_count.load(Ordering::SeqCst),
        "layout_toggle_bg_visible": layout_toggle_bg_visible,
        "active_processes_observed": active_processes,
        "active_title": format!("Spawned/BG ({active_processes} active)"),
        "command_visible": command_visible,
        "bounded_command_seconds": PROCESS_SLEEP_SECONDS,
        "sentinel_processes_cleaned_up": true,
        "artifacts": {
            "screen_before_process": output_dir.join("screen-before-process.txt").display().to_string(),
            "screen_active_process": output_dir.join("screen-active-process.txt").display().to_string()
        }
    });
    let receipt_path = output_dir.join("receipt.json");
    write_json(&receipt_path, &receipt)?;
    Ok(receipt_path)
}

fn ensure_tmux_available() -> Result<(), String> {
    let output = Command::new("tmux")
        .arg("-V")
        .output()
        .map_err(|error| format!("failed to execute tmux: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("tmux -V failed: {}", String::from_utf8_lossy(&output.stderr)))
    }
}

fn clankers_binary() -> Result<PathBuf, String> {
    if let Ok(path) = env::var("CLANKERS_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
        return Err(format!("CLANKERS_BIN does not exist: {}", path.display()));
    }

    let target_dir = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string());
    let default = PathBuf::from(target_dir).join("debug/clankers");
    let status = Command::new("cargo")
        .args(["build", "--bin", "clankers"])
        .status()
        .map_err(|error| format!("failed to run cargo build --bin clankers: {error}"))?;
    if status.success() && default.exists() {
        Ok(default)
    } else {
        Err("clankers binary not found; set CLANKERS_BIN or build cargo --bin clankers".to_string())
    }
}

fn output_dir() -> Result<PathBuf, String> {
    if let Ok(path) = env::var("CLANKERS_BG_DOGFOOD_OUT_DIR") {
        return Ok(PathBuf::from(path));
    }
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system time before UNIX_EPOCH: {error}"))?
        .as_secs();
    Ok(PathBuf::from(format!("target/dogfood/bg-process-tui-{seconds}")))
}

fn start_provider_stub() -> Result<(u16, thread::JoinHandle<()>, Arc<AtomicUsize>), String> {
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|error| format!("failed to bind provider stub: {error}"))?;
    let port = listener.local_addr().map_err(|error| format!("failed to read provider stub addr: {error}"))?.port();
    let request_count = Arc::new(AtomicUsize::new(0));
    let thread_count = Arc::clone(&request_count);
    let handle = thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let index = thread_count.fetch_add(1, Ordering::SeqCst);
            let body = if index == 0 {
                first_model_response()
            } else {
                final_model_response()
            };
            let mut request = [0_u8; 8192];
            let _ = stream.read(&mut request);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    Ok((port, handle, request_count))
}

fn first_model_response() -> String {
    let tool_input = json!({
        "action": "start",
        "command": format!("bash -lc 'echo {SENTINEL_START}; sleep {PROCESS_SLEEP_SECONDS}; echo {SENTINEL_DONE}'"),
        "notify_on_complete": true
    });
    sse(&[
        event(
            "message_start",
            json!({"type":"message_start","message":{"id":"msg-bg-dogfood-1","type":"message","role":"assistant","model":"claude-test","content":[],"stop_reason":null,"usage":{"input_tokens":10,"output_tokens":0}}}),
        ),
        event(
            "content_block_start",
            json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_bg_process_start","name":"process"}}),
        ),
        event(
            "content_block_delta",
            json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":tool_input.to_string()}}),
        ),
        event("content_block_stop", json!({"type":"content_block_stop","index":0})),
        event(
            "message_delta",
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":20}}),
        ),
        event("message_stop", json!({"type":"message_stop"})),
    ])
}

fn final_model_response() -> String {
    sse(&[
        event(
            "message_start",
            json!({"type":"message_start","message":{"id":"msg-bg-dogfood-2","type":"message","role":"assistant","model":"claude-test","content":[],"stop_reason":null,"usage":{"input_tokens":10,"output_tokens":0}}}),
        ),
        event(
            "content_block_start",
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
        ),
        event(
            "content_block_delta",
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"background process dogfood started"}}),
        ),
        event("content_block_stop", json!({"type":"content_block_stop","index":0})),
        event(
            "message_delta",
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":5}}),
        ),
        event("message_stop", json!({"type":"message_stop"})),
    ])
}

fn event(name: &str, data: Value) -> String {
    format!("event: {name}\ndata: {}\n", data)
}

fn sse(events: &[String]) -> String {
    let mut body = events.join("\n");
    body.push('\n');
    body
}

fn launch_clankers_tmux(session: &str, binary: &Path, port: u16) -> Result<(), String> {
    let status = Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            session,
            "-x",
            "140",
            "-y",
            "34",
            "-e",
            "TERM=xterm-256color",
            "-e",
            "RUST_LOG=off",
            "-e",
            "CLANKERS_NO_DAEMON=1",
        ])
        .arg(binary)
        .args([
            "--no-zellij",
            "--no-daemon",
            "--no-session",
            "--provider",
            "anthropic",
            "--api-base",
            &format!("http://127.0.0.1:{port}"),
            "--api-key",
            "dummy",
            "--model",
            "claude-test",
            "--tools",
            "process,procmon",
            "--max-iterations",
            "4",
            "--auto-approve",
        ])
        .status()
        .map_err(|error| format!("failed to launch clankers tmux session: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("tmux new-session failed with status {status}"))
    }
}

fn slash(session: &str, command: &str) -> Result<(), String> {
    send_key(session, "Escape")?;
    sleep_ms(100);
    send_key(session, "i")?;
    sleep_ms(100);
    send_literal(session, command)?;
    sleep_ms(200);
    send_key(session, "Enter")?;
    sleep_ms(600);
    Ok(())
}

fn send_literal(session: &str, text: &str) -> Result<(), String> {
    tmux(["send-keys", "-t", session, "-l", text]).map(|_| ())
}

fn send_key(session: &str, key: &str) -> Result<(), String> {
    tmux(["send-keys", "-t", session, key]).map(|_| ())
}

fn capture_screen(session: &str) -> Result<String, String> {
    tmux(["capture-pane", "-t", session, "-p"])
}

fn wait_for_screen(
    session: &str,
    predicate: impl Fn(&str) -> bool,
    timeout: Duration,
    label: &str,
) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    let mut last = String::new();
    while Instant::now() < deadline {
        last = capture_screen(session)?;
        if predicate(&last) {
            return Ok(last);
        }
        sleep_ms(250);
    }
    Err(format!("timed out waiting for {label}\n--- screen ---\n{last}"))
}

fn active_count(screen: &str) -> Option<usize> {
    let marker = "Spawned/BG (";
    let start = screen.find(marker)? + marker.len();
    let rest = &screen[start..];
    let end = rest.find(" active)")?;
    rest[..end].trim().parse().ok()
}

fn wait_for_sentinel_process_exit(timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let output = Command::new("ps")
            .args(["-eo", "pid=,args="])
            .output()
            .map_err(|error| format!("failed to inspect process table: {error}"))?;
        let listing = String::from_utf8_lossy(&output.stdout);
        let active: Vec<&str> = listing
            .lines()
            .filter(|line| line.contains(SENTINEL_START))
            .filter(|line| !line.contains("check-bg-process-tui-dogfood"))
            .collect();
        if active.is_empty() {
            return Ok(());
        }
        sleep_ms(500);
    }
    Err(format!("sentinel process still running after {}s", timeout.as_secs()))
}

fn tmux<const N: usize>(args: [&str; N]) -> Result<String, String> {
    let output = Command::new("tmux")
        .args(args)
        .output()
        .map_err(|error| format!("failed to execute tmux: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!("tmux command failed: {}", String::from_utf8_lossy(&output.stderr)))
    }
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|error| format!("failed to serialize JSON: {error}"))?;
    fs::write(path, format!("{text}\n")).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn sleep_ms(ms: u64) {
    thread::sleep(Duration::from_millis(ms));
}

struct TmuxSessionGuard {
    session: String,
    started: bool,
}

impl TmuxSessionGuard {
    fn new(session: String) -> Self {
        Self {
            session,
            started: false,
        }
    }

    fn mark_started(&mut self) {
        self.started = true;
    }

    fn close(&mut self) {
        if self.started {
            let _ = send_key(&self.session, "Escape");
            sleep_ms(100);
            let _ = send_literal(&self.session, "q");
            sleep_ms(500);
            let _ = Command::new("tmux").args(["kill-session", "-t", &self.session]).output();
            self.started = false;
        }
    }
}

impl Drop for TmuxSessionGuard {
    fn drop(&mut self) {
        self.close();
    }
}
