#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
serde_json = "1"
---

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
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

use serde_json::json;

const OK: u8 = 0;
const FAIL: u8 = 1;
const SESSION_PREFIX: &str = "clankers-daemon-attach-reconnect-dogfood";
const REPLAY_SENTINEL: &str = "daemon attach reconnect dogfood replay sentinel";

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("daemon attach reconnect dogfood passed: {}", path.display());
            ExitCode::from(OK)
        }
        Err(error) => {
            eprintln!("daemon attach reconnect dogfood failed: {error}");
            ExitCode::from(FAIL)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    ensure_tmux()?;
    let binary = clankers_binary()?;
    let out = output_dir()?;
    fs::create_dir_all(&out).map_err(|e| format!("create {}: {e}", out.display()))?;
    let runtime = out.join("run");
    let home = out.join("home");
    fs::create_dir_all(&runtime).map_err(|e| format!("create runtime: {e}"))?;
    fs::create_dir_all(&home).map_err(|e| format!("create home: {e}"))?;
    let auth_file = out.join("auth.json");
    fs::write(&auth_file, "{}\n").map_err(|e| format!("write auth file: {e}"))?;
    let envs = harness_env(&runtime, &home, &auth_file);
    let (port, _stub, provider_requests) = start_provider_stub()?;
    let api_base = format!("http://127.0.0.1:{port}");
    let mut daemon = DaemonGuard::start(&binary, &envs, &api_base, &out)?;

    wait_for_status(&binary, &envs)?;
    let create = run_clankers(&binary, &envs, &["daemon", "create", "--model", "claude-test"])?;
    let session_id = parse_created_session(&create)?;
    let initial_sessions = count_sessions(&binary, &envs)?;

    let first = format!("{SESSION_PREFIX}-first-{}", std::process::id());
    let second = format!("{SESSION_PREFIX}-second-{}", std::process::id());
    let mut first_guard = TmuxGuard::new(first.clone());
    let mut second_guard = TmuxGuard::new(second.clone());

    launch_attach(&first, &binary, &envs, &session_id)?;
    first_guard.started = true;
    wait_screen(
        &first,
        |s| s.contains("attached to session") || s.contains(&session_id[..8]),
        Duration::from_secs(15),
        "first attach",
    )?;
    send_prompt(&first, "Record the daemon attach reconnect dogfood replay sentinel.")?;
    let first_replay_screen =
        wait_screen(&first, |s| s.contains(REPLAY_SENTINEL), Duration::from_secs(25), "first prompt response")?;
    wait_screen(&first, is_idle_screen, Duration::from_secs(10), "first attach idle after prompt")?;
    slash(&first, "/think high")?;
    let first_think_screen =
        wait_screen(&first, |s| s.contains("Thinking:"), Duration::from_secs(8), "first local thinking parity")?;
    slash(&first, "/detach")?;
    first_guard.close();

    launch_attach(&second, &binary, &envs, &session_id)?;
    second_guard.started = true;
    let replay_screen = wait_screen(
        &second,
        |s| s.contains(REPLAY_SENTINEL),
        Duration::from_secs(20),
        "history replay after reattach",
    )?;
    wait_screen(&second, is_idle_screen, Duration::from_secs(10), "second attach idle after replay")?;
    slash(&second, "/think low")?;
    let second_think_screen = wait_screen(
        &second,
        |s| s.contains("Thinking:"),
        Duration::from_secs(8),
        "post-reattach thinking acknowledgement visibility",
    )?;
    slash(&second, "/detach")?;
    second_guard.close();

    let final_sessions = count_sessions(&binary, &envs)?;
    run_clankers(&binary, &envs, &["daemon", "kill", &session_id])?;
    let daemon_cleaned_up = daemon.stop(&binary, &envs);

    fs::write(out.join("screen-first-replay.txt"), &first_replay_screen)
        .map_err(|e| format!("write first screen: {e}"))?;
    fs::write(out.join("screen-reattach-replay.txt"), &replay_screen)
        .map_err(|e| format!("write reattach screen: {e}"))?;
    fs::write(out.join("screen-first-thinking.txt"), &first_think_screen)
        .map_err(|e| format!("write thinking screen: {e}"))?;
    fs::write(out.join("screen-second-thinking.txt"), &second_think_screen)
        .map_err(|e| format!("write second thinking screen: {e}"))?;

    if initial_sessions != 1 || final_sessions != 1 {
        return Err(format!(
            "expected one daemon session before/after reconnect, got {initial_sessions}/{final_sessions}"
        ));
    }
    if provider_requests.load(Ordering::SeqCst) == 0 {
        return Err("deterministic provider stub was not exercised".to_string());
    }
    let receipt = json!({
        "schema": "clankers.daemon_attach_reconnect_dogfood.receipt.v1",
        "result": "pass",
        "session_id": session_id,
        "first_attach_session": first,
        "second_attach_session": second,
        "deterministic_provider": true,
        "provider_requests": provider_requests.load(Ordering::SeqCst),
        "replayed_history_visible": replay_screen.contains(REPLAY_SENTINEL),
        "session_count_before_reattach": initial_sessions,
        "session_count_after_reattach": final_sessions,
        "session_not_forked": initial_sessions == 1 && final_sessions == 1,
        "post_reattach_ack_visible": second_think_screen.contains("Thinking:"),
        "parity_reset_unit_rail": "cargo test --test daemon_attach_reconnect_dogfood_docs and cargo test attach::local_reconnect_resets_parity_tracker_before_new_events_arrive",
        "daemon_cleaned_up": daemon_cleaned_up,
        "artifacts": {
            "screen_first_replay": out.join("screen-first-replay.txt").display().to_string(),
            "screen_reattach_replay": out.join("screen-reattach-replay.txt").display().to_string(),
            "screen_first_thinking": out.join("screen-first-thinking.txt").display().to_string(),
            "screen_second_thinking": out.join("screen-second-thinking.txt").display().to_string()
        }
    });
    let receipt_path = out.join("receipt.json");
    write_json(&receipt_path, &receipt)?;
    Ok(receipt_path)
}

fn harness_env(runtime: &Path, home: &Path, auth_file: &Path) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("XDG_RUNTIME_DIR".to_string(), runtime.display().to_string()),
        ("HOME".to_string(), home.display().to_string()),
        ("CLANKERS_AUTH_FILE".to_string(), auth_file.display().to_string()),
        ("RUST_LOG".to_string(), "off".to_string()),
        ("NO_COLOR".to_string(), "1".to_string()),
    ])
}

fn run_clankers(binary: &Path, envs: &BTreeMap<String, String>, args: &[&str]) -> Result<String, String> {
    let output = Command::new(binary)
        .args(args)
        .envs(envs)
        .env_remove("CLANKERS_NO_DAEMON")
        .output()
        .map_err(|e| format!("failed to run clankers {args:?}: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(format!(
            "clankers {args:?} failed: status={} stdout={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn wait_for_status(binary: &Path, envs: &BTreeMap<String, String>) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut last = String::new();
    while Instant::now() < deadline {
        match run_clankers(binary, envs, &["daemon", "status"]) {
            Ok(output) if output.contains("Daemon running") => return Ok(()),
            Ok(output) => last = output,
            Err(error) => last = error,
        }
        thread::sleep(Duration::from_millis(200));
    }
    Err(format!("daemon did not become ready: {last}"))
}

fn parse_created_session(output: &str) -> Result<String, String> {
    output
        .lines()
        .find_map(|line| line.strip_prefix("Created session: ").map(str::trim).map(str::to_string))
        .ok_or_else(|| format!("no Created session line in: {output}"))
}

fn count_sessions(binary: &Path, envs: &BTreeMap<String, String>) -> Result<usize, String> {
    let output = run_clankers(binary, envs, &["daemon", "sessions"])?;
    output
        .lines()
        .find_map(|line| line.strip_suffix(" session(s)").and_then(|n| n.trim().parse().ok()))
        .ok_or_else(|| format!("could not count sessions from: {output}"))
}

fn launch_attach(
    session: &str,
    binary: &Path,
    envs: &BTreeMap<String, String>,
    session_id: &str,
) -> Result<(), String> {
    let mut cmd = Command::new("tmux");
    cmd.args(["new-session", "-d", "-s", session, "-x", "132", "-y", "34"]);
    for (key, value) in envs {
        cmd.args(["-e", &format!("{key}={value}")]);
    }
    cmd.args(["-e", "TERM=xterm-256color"]);
    cmd.arg(binary).args(["attach", session_id]);
    let status = cmd.status().map_err(|e| format!("tmux new-session failed: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("tmux new-session exited {status}"))
    }
}

fn send_prompt(session: &str, text: &str) -> Result<(), String> {
    send_key(session, "Escape")?;
    thread::sleep(Duration::from_millis(100));
    send_key(session, "i")?;
    thread::sleep(Duration::from_millis(100));
    send_literal(session, text)?;
    thread::sleep(Duration::from_millis(150));
    send_key(session, "Enter")?;
    Ok(())
}

fn slash(session: &str, command: &str) -> Result<(), String> {
    send_prompt(session, command)
}
fn send_literal(session: &str, text: &str) -> Result<(), String> {
    tmux(["send-keys", "-t", session, "-l", text]).map(|_| ())
}
fn send_key(session: &str, key: &str) -> Result<(), String> {
    tmux(["send-keys", "-t", session, key]).map(|_| ())
}
fn capture(session: &str) -> Result<String, String> {
    tmux(["capture-pane", "-t", session, "-p"])
}

fn is_idle_screen(screen: &str) -> bool {
    screen.contains(" idle |") || screen.contains(" idle ")
}

fn wait_screen(
    session: &str,
    predicate: impl Fn(&str) -> bool,
    timeout: Duration,
    label: &str,
) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    let mut last = String::new();
    while Instant::now() < deadline {
        last = capture(session)?;
        if predicate(&last) {
            return Ok(last);
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err(format!("timed out waiting for {label}; last screen:\n{last}"))
}

fn tmux<const N: usize>(args: [&str; N]) -> Result<String, String> {
    let output = Command::new("tmux").args(args).output().map_err(|e| format!("failed to execute tmux: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(format!("tmux failed: {}", String::from_utf8_lossy(&output.stderr)))
    }
}

struct TmuxGuard {
    session: String,
    started: bool,
}
impl TmuxGuard {
    fn new(session: String) -> Self {
        Self {
            session,
            started: false,
        }
    }
    fn close(&mut self) {
        if self.started {
            let _ = Command::new("tmux").args(["kill-session", "-t", &self.session]).status();
            self.started = false;
        }
    }
}
impl Drop for TmuxGuard {
    fn drop(&mut self) {
        self.close();
    }
}

struct DaemonGuard {
    child: Option<Child>,
}
impl DaemonGuard {
    fn start(binary: &Path, envs: &BTreeMap<String, String>, api_base: &str, out: &Path) -> Result<Self, String> {
        let stdout =
            fs::File::create(out.join("daemon.stdout.log")).map_err(|e| format!("create daemon stdout log: {e}"))?;
        let stderr =
            fs::File::create(out.join("daemon.stderr.log")).map_err(|e| format!("create daemon stderr log: {e}"))?;
        let mut child = Command::new(binary);
        child
            .args([
                "--provider",
                "anthropic",
                "--api-base",
                api_base,
                "--api-key",
                "dummy",
                "--model",
                "claude-test",
            ])
            .args([
                "daemon",
                "start",
                "--allow-all",
                "--heartbeat",
                "0",
                "--max-sessions",
                "4",
            ])
            .envs(envs)
            .env_remove("CLANKERS_NO_DAEMON")
            .stdin(std::process::Stdio::null())
            .stdout(stdout)
            .stderr(stderr);
        let child = child.spawn().map_err(|e| format!("spawn foreground daemon: {e}"))?;
        Ok(Self { child: Some(child) })
    }

    fn stop(&mut self, binary: &Path, envs: &BTreeMap<String, String>) -> bool {
        let command_stop = run_clankers(binary, envs, &["daemon", "stop"]).is_ok();
        let child_clean = if let Some(mut child) = self.child.take() {
            let deadline = Instant::now() + Duration::from_secs(8);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break true,
                    Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(100)),
                    Ok(None) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        break false;
                    }
                    Err(_) => break false,
                }
            }
        } else {
            true
        };
        command_stop && child_clean
    }
}
impl Drop for DaemonGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn ensure_tmux() -> Result<(), String> {
    Command::new("tmux")
        .arg("-V")
        .status()
        .map_err(|e| format!("tmux missing: {e}"))?
        .success()
        .then_some(())
        .ok_or_else(|| "tmux -V failed".to_string())
}

fn clankers_binary() -> Result<PathBuf, String> {
    if let Ok(path) = env::var("CLANKERS_BIN") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }
    let target_dir = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string());
    let default = PathBuf::from(target_dir).join("debug/clankers");
    let status = Command::new("cargo")
        .args(["build", "--bin", "clankers"])
        .status()
        .map_err(|e| format!("cargo build failed to start: {e}"))?;
    if status.success() && default.exists() {
        Ok(default)
    } else {
        Err("clankers binary not found after cargo build".to_string())
    }
}

fn output_dir() -> Result<PathBuf, String> {
    if let Ok(path) = env::var("CLANKERS_DAEMON_ATTACH_DOGFOOD_OUT_DIR") {
        return Ok(PathBuf::from(path));
    }
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("system time before epoch: {e}"))?
        .as_secs();
    Ok(PathBuf::from(format!("target/dogfood/daemon-attach-reconnect-{seconds}")))
}

fn start_provider_stub() -> Result<(u16, thread::JoinHandle<()>, Arc<AtomicUsize>), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("bind provider stub: {e}"))?;
    let port = listener.local_addr().map_err(|e| format!("local addr: {e}"))?.port();
    let count = Arc::new(AtomicUsize::new(0));
    let thread_count = Arc::clone(&count);
    let handle = thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            thread_count.fetch_add(1, Ordering::SeqCst);
            let mut request = [0_u8; 8192];
            let _ = stream.read(&mut request);
            let body = sse_text(REPLAY_SENTINEL);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    Ok((port, handle, count))
}

fn sse_text(text: &str) -> String {
    let events = [
        event(
            "message_start",
            json!({"type":"message_start","message":{"id":"msg-daemon-attach-reconnect","type":"message","role":"assistant","model":"claude-test","content":[],"stop_reason":null,"usage":{"input_tokens":8,"output_tokens":0}}}),
        ),
        event(
            "content_block_start",
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
        ),
        event(
            "content_block_delta",
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":text}}),
        ),
        event("content_block_stop", json!({"type":"content_block_stop","index":0})),
        event(
            "message_delta",
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":8}}),
        ),
        event("message_stop", json!({"type":"message_stop"})),
    ];
    events.join("")
}
fn event(name: &str, data: serde_json::Value) -> String {
    format!("event: {name}\ndata: {data}\n\n")
}
fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    fs::write(path, serde_json::to_string_pretty(value).map_err(|e| format!("json serialize: {e}"))? + "\n")
        .map_err(|e| format!("write {}: {e}", path.display()))
}
