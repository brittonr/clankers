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
use std::net::TcpStream;
use std::path::Path;
use std::path::PathBuf;
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

const ERROR_EXIT: u8 = 1;
const SESSION_PREFIX: &str = "clankers-streaming-dogfood";
const CHUNK_DELAY: Duration = Duration::from_millis(900);
const INTERRUPT_CHUNK_DELAY: Duration = Duration::from_millis(1);
const UNSET_MS: u64 = u64::MAX;

const THINK_ONE: &str = "CLANKERS_THINK_ALPHA";
const THINK_TWO: &str = "CLANKERS_THINK_BETA";
const TOKEN_ONE: &str = "CLANKERS_STREAM_ALPHA";
const TOKEN_TWO: &str = "CLANKERS_STREAM_BETA";
const TOKEN_THREE: &str = "CLANKERS_STREAM_GAMMA";
const INTERRUPT_STREAM_START: &str = "CLANKERS_INTERRUPT_STREAM_START";
const INTERRUPT_FOLLOWUP_ACK: &str = "CLANKERS_INTERRUPT_FOLLOWUP_ACK";

struct StreamTimings {
    interrupt_stream_started_ms: AtomicU64,
    interrupt_stream_completed_ms: AtomicU64,
    followup_request_started_ms: AtomicU64,
}

impl StreamTimings {
    fn new() -> Self {
        Self {
            interrupt_stream_started_ms: AtomicU64::new(UNSET_MS),
            interrupt_stream_completed_ms: AtomicU64::new(UNSET_MS),
            followup_request_started_ms: AtomicU64::new(UNSET_MS),
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(receipt_path) => {
            println!("streaming token recording receipt written to {}", receipt_path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("streaming token recording failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    ensure_tmux_available()?;
    let binary = clankers_binary()?;
    let output_dir = output_dir()?;
    fs::create_dir_all(&output_dir).map_err(|error| format!("failed to create {}: {error}", output_dir.display()))?;

    let (port, _server_thread, request_count, timings) = start_provider_stub()?;
    let session = format!("{SESSION_PREFIX}-{}", std::process::id());
    let mut guard = TmuxSessionGuard::new(session.clone());
    launch_clankers_tmux(&session, &binary, port)?;
    guard.mark_started();

    wait_for_screen(&session, |screen| screen.contains("NORMAL"), Duration::from_secs(20), "initial NORMAL mode")?;
    send_prompt(&session, "Run the delayed-delta dogfood fixture. Do not use tools; just answer with the fixture.")?;

    let started = Instant::now();
    let mut observations = Vec::new();

    let thinking_one = wait_for_screen(
        &session,
        |screen| screen.contains(THINK_ONE) && !screen.contains(THINK_TWO) && screen.contains("streaming…"),
        Duration::from_secs(20),
        "first thinking delta before second thinking delta",
    )?;
    observations.push(record_observation(&output_dir, "thinking-1", started, &thinking_one)?);

    let thinking_two = wait_for_screen(
        &session,
        |screen| screen.contains(THINK_TWO) && !screen.contains(TOKEN_ONE) && screen.contains("streaming…"),
        Duration::from_secs(20),
        "second thinking delta before first text token",
    )?;
    observations.push(record_observation(&output_dir, "thinking-2", started, &thinking_two)?);

    let token_one = wait_for_screen(
        &session,
        |screen| screen.contains(TOKEN_ONE) && !screen.contains(TOKEN_TWO) && screen.contains("streaming…"),
        Duration::from_secs(20),
        "first text token before second text token",
    )?;
    observations.push(record_observation(&output_dir, "token-1", started, &token_one)?);

    let token_two = wait_for_screen(
        &session,
        |screen| screen.contains(TOKEN_TWO) && !screen.contains(TOKEN_THREE) && screen.contains("streaming…"),
        Duration::from_secs(20),
        "second text token before third text token",
    )?;
    observations.push(record_observation(&output_dir, "token-2", started, &token_two)?);

    let token_three = wait_for_screen(
        &session,
        |screen| screen.contains(TOKEN_THREE) && screen.contains("streaming…"),
        Duration::from_secs(20),
        "third text token before finalization",
    )?;
    observations.push(record_observation(&output_dir, "token-3", started, &token_three)?);

    let final_screen = wait_for_screen(
        &session,
        |screen| screen.contains(TOKEN_THREE) && !screen.contains("streaming…"),
        Duration::from_secs(20),
        "finalized response after streaming",
    )?;
    observations.push(record_observation(&output_dir, "final", started, &final_screen)?);

    let mid_stream_input_sent_before_response_returned =
        run_mid_stream_input_check(&session, &output_dir, started, &timings, &mut observations)?;

    let provider_requests = request_count.load(Ordering::SeqCst);
    if provider_requests < 3 {
        return Err(format!(
            "expected at least three provider requests after input-interrupt check, saw {provider_requests}"
        ));
    }

    guard.close();

    let receipt = json!({
        "schema": "clankers.streaming_tokens_recording.receipt.v1",
        "result": "pass",
        "session": session,
        "deterministic_provider_stub": true,
        "provider_requests": provider_requests,
        "chunk_delay_ms": CHUNK_DELAY.as_millis(),
        "thinking_delta_order": [THINK_ONE, THINK_TWO],
        "text_delta_order": [TOKEN_ONE, TOKEN_TWO, TOKEN_THREE],
        "observed_incremental_text": true,
        "observed_incremental_thinking": true,
        "mid_stream_input_sent_before_response_returned": mid_stream_input_sent_before_response_returned,
        "recording_artifacts": observations,
        "input_interrupt_timings_ms": {
            "interrupt_stream_started": timing_value(timings.interrupt_stream_started_ms.load(Ordering::SeqCst)),
            "interrupt_stream_completed": timing_value(timings.interrupt_stream_completed_ms.load(Ordering::SeqCst)),
            "followup_request_started": timing_value(timings.followup_request_started_ms.load(Ordering::SeqCst))
        },
        "assertions": {
            "first_thinking_visible_before_second": true,
            "second_thinking_visible_before_text": true,
            "first_text_visible_before_second": true,
            "second_text_visible_before_third": true,
            "third_text_visible_before_final": true,
            "mid_stream_input_sent_before_response_returned": true,
            "final_response_contains_all_tokens": true,
            "final_response_not_streaming": true
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
    if let Ok(path) = env::var("CLANKERS_STREAMING_DOGFOOD_OUT_DIR") {
        return Ok(PathBuf::from(path));
    }
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system time before UNIX_EPOCH: {error}"))?
        .as_secs();
    Ok(PathBuf::from(format!("target/dogfood/streaming-tokens-{seconds}")))
}

fn run_mid_stream_input_check(
    session: &str,
    output_dir: &Path,
    started: Instant,
    timings: &StreamTimings,
    observations: &mut Vec<Value>,
) -> Result<bool, String> {
    send_prompt(session, "Start the interruptible streaming dogfood fixture and keep streaming until interrupted.")?;
    let active = wait_for_screen(
        session,
        |screen| screen.contains(INTERRUPT_STREAM_START) && screen.contains("streaming…"),
        Duration::from_secs(20),
        "interruptible response streaming before follow-up input",
    )?;
    observations.push(record_observation(output_dir, "interrupt-stream-active", started, &active)?);

    send_prompt(session, "This follow-up prompt must be sent before the previous response returns.")?;
    let followup = wait_for_screen(
        session,
        |screen| screen.contains(INTERRUPT_FOLLOWUP_ACK),
        Duration::from_secs(20),
        "follow-up response after mid-stream input",
    )?;
    observations.push(record_observation(output_dir, "interrupt-followup", started, &followup)?);

    let followup_started = timings.followup_request_started_ms.load(Ordering::SeqCst);
    if followup_started == UNSET_MS {
        return Err("follow-up provider request was never observed".to_string());
    }
    let interrupt_completed = timings.interrupt_stream_completed_ms.load(Ordering::SeqCst);
    if interrupt_completed != UNSET_MS && followup_started >= interrupt_completed {
        return Err(format!(
            "follow-up request started after interrupted stream completed: followup={followup_started}ms completed={interrupt_completed}ms"
        ));
    }
    Ok(true)
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX - 1)
}

fn timing_value(value: u64) -> Value {
    if value == UNSET_MS { Value::Null } else { json!(value) }
}

fn start_provider_stub() -> Result<(u16, thread::JoinHandle<()>, Arc<AtomicUsize>, Arc<StreamTimings>), String> {
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|error| format!("failed to bind provider stub: {error}"))?;
    let port = listener.local_addr().map_err(|error| format!("failed to read provider stub addr: {error}"))?.port();
    let request_count = Arc::new(AtomicUsize::new(0));
    let timings = Arc::new(StreamTimings::new());
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
                    let _ = handle_streaming_response(&mut stream);
                }
                1 => {
                    let _ = handle_interrupt_stream_response(&mut stream, &timings, provider_started);
                }
                2 => {
                    let _ = handle_interrupt_followup_response(&mut stream, &timings, provider_started);
                }
                _ => {
                    let _ = handle_extra_response(&mut stream);
                }
            });
        }
    });
    Ok((port, handle, request_count, timings))
}

fn handle_streaming_response(stream: &mut TcpStream) -> Result<(), String> {
    read_http_request(stream)?;
    write_headers(stream)?;
    stream_event(
        stream,
        "message_start",
        json!({"type":"message_start","message":{"id":"msg-streaming-recording","type":"message","role":"assistant","model":"claude-test","content":[],"stop_reason":null,"usage":{"input_tokens":8,"output_tokens":0}}}),
    )?;
    stream_event(
        stream,
        "content_block_start",
        json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
    )?;
    stream_event(
        stream,
        "content_block_delta",
        json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":format!("{THINK_ONE} ")}}),
    )?;
    sleep_chunk();
    stream_event(
        stream,
        "content_block_delta",
        json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":format!("{THINK_TWO} ")}}),
    )?;
    sleep_chunk();
    stream_event(stream, "content_block_stop", json!({"type":"content_block_stop","index":0}))?;
    stream_event(
        stream,
        "content_block_start",
        json!({"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}),
    )?;
    stream_event(
        stream,
        "content_block_delta",
        json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":format!("{TOKEN_ONE} ")}}),
    )?;
    sleep_chunk();
    stream_event(
        stream,
        "content_block_delta",
        json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":format!("{TOKEN_TWO} ")}}),
    )?;
    sleep_chunk();
    stream_event(
        stream,
        "content_block_delta",
        json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":TOKEN_THREE}}),
    )?;
    sleep_chunk();
    stream_event(stream, "content_block_stop", json!({"type":"content_block_stop","index":1}))?;
    stream_event(
        stream,
        "message_delta",
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":8,"output_tokens":5}}),
    )?;
    stream_event(stream, "message_stop", json!({"type":"message_stop"}))?;
    Ok(())
}

fn handle_interrupt_stream_response(
    stream: &mut TcpStream,
    timings: &StreamTimings,
    provider_started: Instant,
) -> Result<(), String> {
    read_http_request(stream)?;
    timings.interrupt_stream_started_ms.store(elapsed_ms(provider_started), Ordering::SeqCst);
    write_headers(stream)?;
    stream_event(
        stream,
        "message_start",
        json!({"type":"message_start","message":{"id":"msg-interrupt-stream","type":"message","role":"assistant","model":"claude-test","content":[],"stop_reason":null,"usage":{"input_tokens":8,"output_tokens":0}}}),
    )?;
    stream_event(
        stream,
        "content_block_start",
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
    )?;
    stream_event(
        stream,
        "content_block_delta",
        json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":format!("{INTERRUPT_STREAM_START} ")}}),
    )?;
    for _ in 0..4000 {
        stream_event(
            stream,
            "content_block_delta",
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"."}}),
        )?;
        thread::sleep(INTERRUPT_CHUNK_DELAY);
    }
    stream_event(stream, "content_block_stop", json!({"type":"content_block_stop","index":0}))?;
    stream_event(
        stream,
        "message_delta",
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":8,"output_tokens":4000}}),
    )?;
    stream_event(stream, "message_stop", json!({"type":"message_stop"}))?;
    timings.interrupt_stream_completed_ms.store(elapsed_ms(provider_started), Ordering::SeqCst);
    Ok(())
}

fn handle_interrupt_followup_response(
    stream: &mut TcpStream,
    timings: &StreamTimings,
    provider_started: Instant,
) -> Result<(), String> {
    read_http_request(stream)?;
    timings.followup_request_started_ms.store(elapsed_ms(provider_started), Ordering::SeqCst);
    write_headers(stream)?;
    stream_event(
        stream,
        "message_start",
        json!({"type":"message_start","message":{"id":"msg-interrupt-followup","type":"message","role":"assistant","model":"claude-test","content":[],"stop_reason":null,"usage":{"input_tokens":8,"output_tokens":0}}}),
    )?;
    stream_event(
        stream,
        "content_block_start",
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
    )?;
    stream_event(
        stream,
        "content_block_delta",
        json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":INTERRUPT_FOLLOWUP_ACK}}),
    )?;
    stream_event(stream, "content_block_stop", json!({"type":"content_block_stop","index":0}))?;
    stream_event(
        stream,
        "message_delta",
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":8,"output_tokens":1}}),
    )?;
    stream_event(stream, "message_stop", json!({"type":"message_stop"}))?;
    Ok(())
}

fn handle_extra_response(stream: &mut TcpStream) -> Result<(), String> {
    read_http_request(stream)?;
    write_headers(stream)?;
    stream_event(
        stream,
        "message_start",
        json!({"type":"message_start","message":{"id":"msg-extra","type":"message","role":"assistant","model":"claude-test","content":[],"stop_reason":null,"usage":{"input_tokens":1,"output_tokens":0}}}),
    )?;
    stream_event(
        stream,
        "content_block_start",
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
    )?;
    stream_event(
        stream,
        "content_block_delta",
        json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"unexpected extra request"}}),
    )?;
    stream_event(stream, "content_block_stop", json!({"type":"content_block_stop","index":0}))?;
    stream_event(
        stream,
        "message_delta",
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}}),
    )?;
    stream_event(stream, "message_stop", json!({"type":"message_stop"}))?;
    Ok(())
}

fn read_http_request(stream: &mut TcpStream) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("failed to set provider stub read timeout: {error}"))?;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut header_end = None;
    let mut expected_len = None;
    loop {
        let read = stream.read(&mut buffer).map_err(|error| format!("failed to read provider request: {error}"))?;
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
        .map_err(|error| format!("failed to write provider response headers: {error}"))?;
    stream.flush().map_err(|error| format!("failed to flush provider response headers: {error}"))
}

fn stream_event(stream: &mut TcpStream, name: &str, data: Value) -> Result<(), String> {
    let event = format!("event: {name}\ndata: {data}\n\n");
    stream
        .write_all(event.as_bytes())
        .map_err(|error| format!("failed to write provider stream event {name}: {error}"))?;
    stream.flush().map_err(|error| format!("failed to flush provider stream event {name}: {error}"))
}

fn sleep_chunk() {
    thread::sleep(CHUNK_DELAY);
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
            "--system-prompt",
            "You are a deterministic streaming dogfood assistant.",
            "--tools",
            "none",
            "--max-iterations",
            "1",
        ])
        .status()
        .map_err(|error| format!("failed to launch clankers tmux session: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("tmux new-session failed with status {status}"))
    }
}

fn send_prompt(session: &str, prompt: &str) -> Result<(), String> {
    send_key(session, "Escape")?;
    sleep_ms(100);
    send_key(session, "i")?;
    sleep_ms(100);
    send_literal(session, prompt)?;
    sleep_ms(200);
    send_key(session, "Enter")?;
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
        sleep_ms(100);
    }
    Err(format!("timed out waiting for {label}\n--- screen ---\n{last}"))
}

fn record_observation(output_dir: &Path, label: &str, started: Instant, screen: &str) -> Result<Value, String> {
    let screen_path = output_dir.join(format!("screen-{label}.txt"));
    fs::write(&screen_path, screen).map_err(|error| format!("failed to write {}: {error}", screen_path.display()))?;
    Ok(json!({
        "label": label,
        "elapsed_ms": started.elapsed().as_millis(),
        "screen": screen_path.display().to_string(),
        "contains": {
            "thinking_one": screen.contains(THINK_ONE),
            "thinking_two": screen.contains(THINK_TWO),
            "token_one": screen.contains(TOKEN_ONE),
            "token_two": screen.contains(TOKEN_TWO),
            "token_three": screen.contains(TOKEN_THREE),
            "interrupt_stream_start": screen.contains(INTERRUPT_STREAM_START),
            "interrupt_followup_ack": screen.contains(INTERRUPT_FOLLOWUP_ACK),
            "streaming_indicator": screen.contains("streaming…"),
            "finalized": !screen.contains("streaming…"),
        }
    }))
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
