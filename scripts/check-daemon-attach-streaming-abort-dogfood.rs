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
use std::net::TcpStream;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde_json::Value;
use serde_json::json;

const OK: u8 = 0;
const FAIL: u8 = 1;
const SESSION_PREFIX: &str = "clankers-daemon-attach-streaming-abort-dogfood";
const UNSET_MS: u64 = u64::MAX;
const INTERRUPT_STREAM_START: &str = "DAEMON_ATTACH_ABORT_START";
const FOLLOWUP_ACK: &str = "DAEMON_ATTACH_ABORT_FOLLOWUP_ACK";
const BUSY_REJECTION: &str = "A prompt is already in progress";
const STREAM_CHUNK_DELAY: Duration = Duration::from_millis(5);
const STREAM_CHUNKS: usize = 20_000;

struct Timings {
    stream_started_ms: AtomicU64,
    stream_completed_ms: AtomicU64,
    stream_closed_early_ms: AtomicU64,
    followup_started_ms: AtomicU64,
}

impl Timings {
    fn new() -> Self {
        Self {
            stream_started_ms: AtomicU64::new(UNSET_MS),
            stream_completed_ms: AtomicU64::new(UNSET_MS),
            stream_closed_early_ms: AtomicU64::new(UNSET_MS),
            followup_started_ms: AtomicU64::new(UNSET_MS),
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("daemon attach streaming abort dogfood passed: {}", path.display());
            ExitCode::from(OK)
        }
        Err(error) => {
            eprintln!("daemon attach streaming abort dogfood failed: {error}");
            ExitCode::from(FAIL)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    ensure_tmux()?;
    let binary = clankers_binary()?;
    let out = output_dir()?;
    fs::create_dir_all(&out).map_err(|error| format!("create {}: {error}", out.display()))?;
    let runtime = out.join("run");
    let home = out.join("home");
    fs::create_dir_all(&runtime).map_err(|error| format!("create runtime dir: {error}"))?;
    fs::create_dir_all(&home).map_err(|error| format!("create home dir: {error}"))?;
    let auth_file = out.join("auth.json");
    fs::write(&auth_file, "{}\n").map_err(|error| format!("write auth file: {error}"))?;
    let envs = harness_env(&runtime, &home, &auth_file);
    let (port, _stub, provider_requests, timings) = start_provider_stub()?;
    let api_base = format!("http://127.0.0.1:{port}");
    let mut daemon = DaemonGuard::start(&binary, &envs, &api_base, &out)?;

    wait_for_status(&binary, &envs)?;
    let create = run_clankers(&binary, &envs, &["daemon", "create", "--model", "claude-test"])?;
    let session_id = parse_created_session(&create)?;

    let tmux_session = format!("{SESSION_PREFIX}-{}", std::process::id());
    let mut tmux_guard = TmuxGuard::new(tmux_session.clone());
    launch_attach(&tmux_session, &binary, &envs, &session_id)?;
    tmux_guard.started = true;
    wait_screen(
        &tmux_session,
        |screen| screen.contains("attached to session") || screen.contains(&session_id[..8]) || screen.contains("NORMAL"),
        Duration::from_secs(20),
        "attached TUI ready",
    )?;

    let started = Instant::now();
    let mut observations = Vec::new();
    send_prompt(&tmux_session, "Start the daemon attach streaming abort dogfood response and keep streaming.")?;
    let active = wait_screen(
        &tmux_session,
        |screen| screen.contains("streaming…"),
        Duration::from_secs(25),
        "daemon-attached stream active before abort",
    )?;
    observations.push(record_screen(&out, "stream-active", started, &active)?);

    send_prompt(&tmux_session, "This follow-up must be accepted before the daemon's interrupted stream returns.")?;
    let followup = wait_screen(
        &tmux_session,
        |screen| screen.contains(FOLLOWUP_ACK),
        Duration::from_secs(25),
        "follow-up response after mid-stream abort",
    )?;
    observations.push(record_screen(&out, "followup-ack", started, &followup)?);

    let request_count = provider_requests.load(Ordering::SeqCst);
    let followup_started = timings.followup_started_ms.load(Ordering::SeqCst);
    let stream_completed = timings.stream_completed_ms.load(Ordering::SeqCst);
    let stream_closed_early = timings.stream_closed_early_ms.load(Ordering::SeqCst);
    if request_count < 2 {
        return Err(format!("expected at least two provider requests, saw {request_count}"));
    }
    if followup_started == UNSET_MS {
        return Err("follow-up provider request was never observed".to_string());
    }
    if stream_completed != UNSET_MS && followup_started >= stream_completed {
        return Err(format!(
            "follow-up request started after interrupted daemon stream completed: followup={followup_started}ms completed={stream_completed}ms"
        ));
    }
    if followup.contains(BUSY_REJECTION) {
        return Err("attached follow-up hit daemon busy rejection instead of abort/replay".to_string());
    }

    slash(&tmux_session, "/detach")?;
    tmux_guard.close();
    run_clankers(&binary, &envs, &["daemon", "kill", &session_id])?;
    let daemon_cleaned_up = daemon.stop(&binary, &envs);

    let receipt = json!({
        "schema": "clankers.daemon_attach_streaming_abort_dogfood.receipt.v1",
        "result": "pass",
        "session_id": session_id,
        "attach_tmux_session": tmux_session,
        "deterministic_provider": true,
        "provider_requests": request_count,
        "stream_chunk_delay_ms": STREAM_CHUNK_DELAY.as_millis(),
        "stream_chunks_if_not_aborted": STREAM_CHUNKS,
        "mid_stream_abort_processed_before_provider_returned": true,
        "followup_request_started_before_stream_completed": stream_completed == UNSET_MS || followup_started < stream_completed,
        "busy_rejection_visible": false,
        "daemon_cleaned_up": daemon_cleaned_up,
        "timings_ms": {
            "stream_started": timing_value(timings.stream_started_ms.load(Ordering::SeqCst)),
            "stream_completed": timing_value(stream_completed),
            "stream_closed_early": timing_value(stream_closed_early),
            "followup_started": timing_value(followup_started)
        },
        "assertions": {
            "stream_visible_before_abort": true,
            "followup_visible_after_abort": true,
            "provider_requests_at_least_two": true,
            "followup_request_started_before_stream_completed": true,
            "no_busy_rejection": true
        },
        "artifacts": observations
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

fn start_provider_stub() -> Result<(u16, thread::JoinHandle<()>, Arc<AtomicUsize>, Arc<Timings>), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| format!("bind provider stub: {error}"))?;
    let port = listener.local_addr().map_err(|error| format!("provider local addr: {error}"))?.port();
    let request_count = Arc::new(AtomicUsize::new(0));
    let timings = Arc::new(Timings::new());
    let thread_count = Arc::clone(&request_count);
    let thread_timings = Arc::clone(&timings);
    let provider_started = Instant::now();
    let handle = thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let index = thread_count.fetch_add(1, Ordering::SeqCst);
            let timings = Arc::clone(&thread_timings);
            thread::spawn(move || match index {
                0 => {
                    let _ = handle_interruptible_stream(&mut stream, &timings, provider_started);
                }
                1 => {
                    let _ = handle_followup(&mut stream, &timings, provider_started);
                }
                _ => {
                    let _ = handle_extra(&mut stream);
                }
            });
        }
    });
    Ok((port, handle, request_count, timings))
}

fn handle_interruptible_stream(
    stream: &mut TcpStream,
    timings: &Timings,
    provider_started: Instant,
) -> Result<(), String> {
    read_http_request(stream)?;
    timings.stream_started_ms.store(elapsed_ms(provider_started), Ordering::SeqCst);
    write_headers(stream)?;
    stream_event(
        stream,
        "message_start",
        json!({"type":"message_start","message":{"id":"msg-daemon-attach-abort-stream","type":"message","role":"assistant","model":"claude-test","content":[],"stop_reason":null,"usage":{"input_tokens":8,"output_tokens":0}}}),
        timings,
        provider_started,
    )?;
    stream_event(
        stream,
        "content_block_start",
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
        timings,
        provider_started,
    )?;
    stream_event(
        stream,
        "content_block_delta",
        json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":format!("{INTERRUPT_STREAM_START} ")}}),
        timings,
        provider_started,
    )?;
    thread::sleep(Duration::from_millis(1_000));
    for _ in 0..STREAM_CHUNKS {
        stream_event(
            stream,
            "content_block_delta",
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"."}}),
            timings,
            provider_started,
        )?;
        thread::sleep(STREAM_CHUNK_DELAY);
    }
    stream_event(stream, "content_block_stop", json!({"type":"content_block_stop","index":0}), timings, provider_started)?;
    stream_event(
        stream,
        "message_delta",
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":8,"output_tokens":STREAM_CHUNKS}}),
        timings,
        provider_started,
    )?;
    stream_event(stream, "message_stop", json!({"type":"message_stop"}), timings, provider_started)?;
    timings.stream_completed_ms.store(elapsed_ms(provider_started), Ordering::SeqCst);
    Ok(())
}

fn handle_followup(stream: &mut TcpStream, timings: &Timings, provider_started: Instant) -> Result<(), String> {
    read_http_request(stream)?;
    timings.followup_started_ms.store(elapsed_ms(provider_started), Ordering::SeqCst);
    write_headers(stream)?;
    stream_event_without_timing(
        stream,
        "message_start",
        json!({"type":"message_start","message":{"id":"msg-daemon-attach-abort-followup","type":"message","role":"assistant","model":"claude-test","content":[],"stop_reason":null,"usage":{"input_tokens":8,"output_tokens":0}}}),
    )?;
    stream_event_without_timing(
        stream,
        "content_block_start",
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
    )?;
    stream_event_without_timing(
        stream,
        "content_block_delta",
        json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":FOLLOWUP_ACK}}),
    )?;
    stream_event_without_timing(stream, "content_block_stop", json!({"type":"content_block_stop","index":0}))?;
    stream_event_without_timing(
        stream,
        "message_delta",
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":8,"output_tokens":1}}),
    )?;
    stream_event_without_timing(stream, "message_stop", json!({"type":"message_stop"}))?;
    Ok(())
}

fn handle_extra(stream: &mut TcpStream) -> Result<(), String> {
    read_http_request(stream)?;
    write_headers(stream)?;
    stream_event_without_timing(
        stream,
        "message_start",
        json!({"type":"message_start","message":{"id":"msg-daemon-attach-abort-extra","type":"message","role":"assistant","model":"claude-test","content":[],"stop_reason":null,"usage":{"input_tokens":1,"output_tokens":0}}}),
    )?;
    stream_event_without_timing(
        stream,
        "content_block_start",
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
    )?;
    stream_event_without_timing(
        stream,
        "content_block_delta",
        json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"unexpected extra daemon attach abort request"}}),
    )?;
    stream_event_without_timing(stream, "content_block_stop", json!({"type":"content_block_stop","index":0}))?;
    stream_event_without_timing(
        stream,
        "message_delta",
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}}),
    )?;
    stream_event_without_timing(stream, "message_stop", json!({"type":"message_stop"}))?;
    Ok(())
}

fn read_http_request(stream: &mut TcpStream) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("set provider read timeout: {error}"))?;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut header_end = None;
    let mut expected_len = None;
    loop {
        let read = stream.read(&mut buffer).map_err(|error| format!("read provider request: {error}"))?;
        if read == 0 {
            return Ok(());
        }
        request.extend_from_slice(&buffer[..read]);
        if header_end.is_none() {
            header_end = find_header_end(&request);
            if let Some(end) = header_end {
                expected_len = parse_content_length(&request[..end]);
            }
        }
        if let Some(end) = header_end {
            let body_len = request.len().saturating_sub(end);
            if expected_len.is_none_or(|len| body_len >= len) {
                return Ok(());
            }
        }
    }
}

fn find_header_end(request: &[u8]) -> Option<usize> {
    request.windows(4).position(|window| window == b"\r\n\r\n").map(|index| index + 4)
}

fn parse_content_length(headers: &[u8]) -> Option<usize> {
    let text = String::from_utf8_lossy(headers);
    text.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse().ok()).flatten()
    })
}

fn write_headers(stream: &mut TcpStream) -> Result<(), String> {
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n")
        .map_err(|error| format!("write provider headers: {error}"))?;
    stream.flush().map_err(|error| format!("flush provider headers: {error}"))
}

fn stream_event(
    stream: &mut TcpStream,
    name: &str,
    data: Value,
    timings: &Timings,
    provider_started: Instant,
) -> Result<(), String> {
    match stream_event_without_timing(stream, name, data) {
        Ok(()) => Ok(()),
        Err(error) => {
            timings.stream_closed_early_ms.store(elapsed_ms(provider_started), Ordering::SeqCst);
            Err(error)
        }
    }
}

fn stream_event_without_timing(stream: &mut TcpStream, name: &str, data: Value) -> Result<(), String> {
    let event = format!("event: {name}\ndata: {data}\n\n");
    stream
        .write_all(event.as_bytes())
        .map_err(|error| format!("write provider stream event {name}: {error}"))?;
    stream.flush().map_err(|error| format!("flush provider stream event {name}: {error}"))
}

fn run_clankers(binary: &Path, envs: &BTreeMap<String, String>, args: &[&str]) -> Result<String, String> {
    let output = Command::new(binary)
        .args(args)
        .envs(envs)
        .env_remove("CLANKERS_NO_DAEMON")
        .output()
        .map_err(|error| format!("failed to run clankers {args:?}: {error}"))?;
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
    let deadline = Instant::now() + Duration::from_secs(15);
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

fn launch_attach(
    session: &str,
    binary: &Path,
    envs: &BTreeMap<String, String>,
    session_id: &str,
) -> Result<(), String> {
    let mut cmd = Command::new("tmux");
    cmd.args(["new-session", "-d", "-s", session, "-x", "140", "-y", "34"]);
    for (key, value) in envs {
        cmd.args(["-e", &format!("{key}={value}")]);
    }
    cmd.args(["-e", "TERM=xterm-256color"]);
    cmd.arg(binary).args(["attach", session_id]);
    let status = cmd.status().map_err(|error| format!("tmux new-session failed: {error}"))?;
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
    thread::sleep(Duration::from_millis(200));
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
        thread::sleep(Duration::from_millis(200));
    }
    Err(format!("timed out waiting for {label}; last screen:\n{last}"))
}

fn tmux<const N: usize>(args: [&str; N]) -> Result<String, String> {
    let output = Command::new("tmux").args(args).output().map_err(|error| format!("execute tmux: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(format!("tmux failed: {}", String::from_utf8_lossy(&output.stderr)))
    }
}

fn record_screen(output_dir: &Path, label: &str, started: Instant, screen: &str) -> Result<Value, String> {
    let screen_path = output_dir.join(format!("screen-{label}.txt"));
    fs::write(&screen_path, screen).map_err(|error| format!("write {}: {error}", screen_path.display()))?;
    Ok(json!({
        "label": label,
        "elapsed_ms": started.elapsed().as_millis(),
        "screen": screen_path.display().to_string(),
        "contains": {
            "stream_start": screen.contains(INTERRUPT_STREAM_START),
            "followup_ack": screen.contains(FOLLOWUP_ACK),
            "busy_rejection": screen.contains(BUSY_REJECTION),
            "streaming_indicator": screen.contains("streaming…")
        }
    }))
}

fn timing_value(value: u64) -> Value {
    if value == UNSET_MS { Value::Null } else { json!(value) }
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX - 1)
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|error| format!("json serialize: {error}"))?;
    fs::write(path, format!("{text}\n")).map_err(|error| format!("write {}: {error}", path.display()))
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
        let stdout = fs::File::create(out.join("daemon.stdout.log")).map_err(|error| format!("daemon stdout log: {error}"))?;
        let stderr = fs::File::create(out.join("daemon.stderr.log")).map_err(|error| format!("daemon stderr log: {error}"))?;
        let child = Command::new(binary)
            .args([
                "--provider",
                "anthropic",
                "--api-base",
                api_base,
                "--api-key",
                "dummy",
                "--model",
                "claude-test",
                "--system-prompt",
                "You are a deterministic daemon attach streaming abort dogfood assistant.",
                "--tools",
                "none",
                "--max-iterations",
                "1",
            ])
            .args(["daemon", "start", "--allow-all", "--heartbeat", "0", "--max-sessions", "4"])
            .envs(envs)
            .env_remove("CLANKERS_NO_DAEMON")
            .stdin(std::process::Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .spawn()
            .map_err(|error| format!("spawn foreground daemon: {error}"))?;
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
        .map_err(|error| format!("tmux missing: {error}"))?
        .success()
        .then_some(())
        .ok_or_else(|| "tmux -V failed".to_string())
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
        .map_err(|error| format!("cargo build failed to start: {error}"))?;
    if status.success() && default.exists() {
        Ok(default)
    } else {
        Err("clankers binary not found after cargo build".to_string())
    }
}

fn output_dir() -> Result<PathBuf, String> {
    if let Ok(path) = env::var("CLANKERS_DAEMON_ATTACH_ABORT_DOGFOOD_OUT_DIR") {
        return Ok(PathBuf::from(path));
    }
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system time before UNIX_EPOCH: {error}"))?
        .as_secs();
    Ok(PathBuf::from(format!("target/dogfood/daemon-attach-streaming-abort-{seconds}")))
}
