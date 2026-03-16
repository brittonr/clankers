//! Daemon command handlers — start/stop/status/sessions/logs.

use std::io::BufRead;
use std::io::Seek;

use clankers_controller::transport;
use clankers_protocol::control::ControlCommand;
use clankers_protocol::control::ControlResponse;
use clankers_protocol::frame;
use tokio::net::UnixStream;

use crate::cli::DaemonAction;
use crate::commands::CommandContext;
use crate::error::Result;

/// Dispatch a daemon subcommand.
pub async fn dispatch(ctx: &CommandContext, action: DaemonAction) -> Result<()> {
    match action {
        DaemonAction::Start {
            background,
            tags,
            allow_all,
            matrix,
            heartbeat,
            max_sessions,
        } => {
            if background {
                start_background(ctx, tags, allow_all, matrix, heartbeat, max_sessions)?;
            } else {
                start_foreground(ctx, tags, allow_all, matrix, heartbeat, max_sessions).await?;
            }
        }
        DaemonAction::Stop => stop().await?,
        DaemonAction::Status => status().await?,
        DaemonAction::Sessions { all } => dispatch_sessions(all).await?,
        DaemonAction::Create { model, system_prompt } => create(model, system_prompt).await?,
        DaemonAction::Kill { session_id } => kill(session_id).await?,
        DaemonAction::Logs { follow, lines } => logs(follow, lines)?,
    }
    Ok(())
}

// ── Start ───────────────────────────────────────────────────────────────────

/// Run the daemon in the foreground (blocks until Ctrl+C).
async fn start_foreground(
    ctx: &CommandContext,
    tags: Vec<String>,
    allow_all: bool,
    matrix: bool,
    heartbeat: u64,
    max_sessions: usize,
) -> Result<()> {
    // Bail if a daemon is already running
    if let Some(pid) = transport::running_daemon_pid() {
        return Err(crate::error::Error::Provider {
            message: format!(
                "Daemon already running (PID {pid}).\nStop it first: clankers daemon stop"
            ),
        });
    }

    let provider = crate::provider::discovery::build_router(
        ctx.api_key.as_deref(),
        ctx.api_base.clone(),
        &ctx.paths.global_auth,
        ctx.paths.pi_auth.as_deref(),
        ctx.account.as_deref(),
    )?;

    let process_monitor = {
        let config = crate::procmon::ProcessMonitorConfig::default();
        let monitor = std::sync::Arc::new(crate::procmon::ProcessMonitor::new(config, None));
        monitor.clone().start();
        monitor
    };
    let env = crate::modes::common::ToolEnv {
        process_monitor: Some(process_monitor),
        ..Default::default()
    };
    let tiered = crate::modes::common::build_tiered_tools(&env);
    let tool_set = crate::modes::common::ToolSet::new(tiered, [
        crate::modes::common::ToolTier::Core,
        crate::modes::common::ToolTier::Orchestration,
        crate::modes::common::ToolTier::Specialty,
        crate::modes::common::ToolTier::Matrix,
    ]);
    let tools = tool_set.active_tools();

    let config = crate::modes::daemon::DaemonConfig {
        model: ctx.model.clone(),
        system_prompt: ctx.system_prompt.clone(),
        settings: ctx.settings.clone(),
        tags,
        allow_all,
        enable_matrix: matrix,
        heartbeat_secs: heartbeat,
        max_sessions,
        ..Default::default()
    };

    crate::modes::daemon::run_daemon(provider, tools, config, &ctx.paths).await?;
    Ok(())
}

/// Fork to background, redirect output to log file, and exit the parent.
fn start_background(
    ctx: &CommandContext,
    tags: Vec<String>,
    allow_all: bool,
    matrix: bool,
    heartbeat: u64,
    max_sessions: usize,
) -> Result<()> {
    // Bail if a daemon is already running
    if let Some(pid) = transport::running_daemon_pid() {
        return Err(crate::error::Error::Provider {
            message: format!(
                "Daemon already running (PID {pid}).\nStop it first: clankers daemon stop"
            ),
        });
    }

    // Ensure socket dir exists for the log file
    let sock_dir = transport::socket_dir();
    std::fs::create_dir_all(&sock_dir).map_err(|e| crate::error::Error::Io { source: e })?;

    let log_path = transport::daemon_log_path();

    // Build the command to re-exec ourselves in foreground mode.
    // Top-level flags (--model, --log-file, --log-level) go BEFORE the
    // subcommand; daemon start flags go after.
    let exe = std::env::current_exe().map_err(|e| crate::error::Error::Io { source: e })?;
    let mut cmd = std::process::Command::new(exe);

    // Forward model and logging as top-level flags (before subcommand)
    cmd.args(["--model", &ctx.model]);
    cmd.arg("--log-file").arg(&log_path);
    cmd.arg("--log-level").arg("info");

    cmd.arg("daemon").arg("start");

    // Forward daemon start flags
    if allow_all {
        cmd.arg("--allow-all");
    }
    if matrix {
        cmd.arg("--matrix");
    }
    if heartbeat != 60 {
        cmd.args(["--heartbeat", &heartbeat.to_string()]);
    }
    if max_sessions != 32 {
        cmd.args(["--max-sessions", &max_sessions.to_string()]);
    }
    if !tags.is_empty() {
        cmd.args(["--tags", &tags.join(",")]);
    }

    // Redirect stdout/stderr to log file, detach stdin
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| crate::error::Error::Io { source: e })?;
    let log_err = log_file
        .try_clone()
        .map_err(|e| crate::error::Error::Io { source: e })?;

    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(log_file);
    cmd.stderr(log_err);

    let child = cmd.spawn().map_err(|e| crate::error::Error::Io { source: e })?;

    println!("Daemon starting in background (PID {}).", child.id());
    println!("  Logs: {}", log_path.display());
    println!("  Stop: clankers daemon stop");

    // Give it a moment to start, then check if it's alive
    std::thread::sleep(std::time::Duration::from_millis(500));
    let pid = child.id();
    if is_process_alive(pid) {
        println!("  Status: running ✓");
    } else {
        eprintln!("  Status: exited (check logs for errors)");
    }

    Ok(())
}

/// Ensure a daemon is running. If not, start one in background with defaults.
/// Used by `--auto-daemon` on attach.
pub async fn ensure_daemon_running() -> Result<()> {
    if transport::running_daemon_pid().is_some() {
        // Already running — verify socket responds
        if send_control(ControlCommand::Status).await.is_ok() {
            return Ok(());
        }
        // PID alive but socket dead — stale, try starting fresh
        eprintln!("Stale daemon detected, starting fresh...");
    }

    eprintln!("Starting daemon in background...");
    let exe = std::env::current_exe().map_err(|e| crate::error::Error::Io { source: e })?;

    let sock_dir = transport::socket_dir();
    std::fs::create_dir_all(&sock_dir).map_err(|e| crate::error::Error::Io { source: e })?;
    let log_path = transport::daemon_log_path();

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| crate::error::Error::Io { source: e })?;
    let log_err = log_file
        .try_clone()
        .map_err(|e| crate::error::Error::Io { source: e })?;

    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--log-file").arg(&log_path);
    cmd.arg("--log-level").arg("info");
    cmd.args(["daemon", "start"]);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(log_file);
    cmd.stderr(log_err);

    let child = cmd.spawn().map_err(|e| crate::error::Error::Io { source: e })?;
    let pid = child.id();

    // Wait for the control socket to become responsive
    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        if send_control(ControlCommand::Status).await.is_ok() {
            eprintln!("Daemon running (PID {pid}).");
            return Ok(());
        }
    }

    if is_process_alive(pid) {
        eprintln!("Daemon started (PID {pid}) but socket not yet responsive.");
        Ok(())
    } else {
        Err(crate::error::Error::Provider {
            message: format!("Daemon failed to start. Check logs: {}", log_path.display()),
        })
    }
}

// ── Stop ────────────────────────────────────────────────────────────────────

async fn stop() -> Result<()> {
    // Check PID first for a better error message
    let pid = transport::running_daemon_pid();
    if pid.is_none() {
        println!("No daemon running.");
        return Ok(());
    }

    let resp = send_control(ControlCommand::Shutdown).await?;
    match resp {
        ControlResponse::ShuttingDown => {
            println!("Daemon shutting down (PID {}).", pid.unwrap());
        }
        ControlResponse::Error { message } => {
            eprintln!("Error: {message}");
        }
        other => {
            eprintln!("Unexpected response: {other:?}");
        }
    }
    Ok(())
}

// ── Status ──────────────────────────────────────────────────────────────────

async fn status() -> Result<()> {
    match transport::running_daemon_pid() {
        None => {
            println!("Daemon is not running.");
            return Ok(());
        }
        Some(pid) => {
            // Try the control socket for rich info
            match send_control(ControlCommand::Status).await {
                Ok(ControlResponse::Status(s)) => {
                    println!("Daemon running (PID {pid})");
                    println!("  Uptime:   {}s", format_duration(s.uptime_secs));
                    println!("  Sessions: {}", s.session_count);
                    println!("  Clients:  {}", s.total_clients);
                    println!("  Socket:   {}", transport::control_socket_path().display());
                    println!("  Logs:     {}", transport::daemon_log_path().display());
                }
                Ok(ControlResponse::Error { message }) => {
                    eprintln!("Daemon running (PID {pid}) but returned error: {message}");
                }
                Ok(other) => {
                    eprintln!("Daemon running (PID {pid}) — unexpected response: {other:?}");
                }
                Err(_) => {
                    // PID alive but socket not responding
                    println!("Daemon running (PID {pid}) but control socket unreachable.");
                    println!("  Socket:   {}", transport::control_socket_path().display());
                }
            }
        }
    }
    Ok(())
}

fn format_duration(secs: f64) -> String {
    let s = secs as u64;
    if s < 60 {
        format!("{s}")
    } else if s < 3600 {
        format!("{}m {}s", s / 60, s % 60)
    } else {
        format!("{}h {}m", s / 3600, (s % 3600) / 60)
    }
}

// ── Sessions ────────────────────────────────────────────────────────────────

/// `clankers ps` / `clankers daemon sessions` — compact session listing.
pub async fn dispatch_sessions(show_all: bool) -> Result<()> {
    let resp = send_control(ControlCommand::ListSessions).await?;
    match resp {
        ControlResponse::Sessions(sessions) => {
            if sessions.is_empty() {
                println!("No active sessions.");
                return Ok(());
            }
            if show_all {
                println!(
                    "{:<10} {:<28} {:>5} {:>7} {:<20} SOCKET",
                    "SESSION", "MODEL", "TURNS", "CLIENTS", "LAST ACTIVE"
                );
            } else {
                println!(
                    "{:<10} {:<28} {:>5} {:>7} LAST ACTIVE",
                    "SESSION", "MODEL", "TURNS", "CLIENTS"
                );
            }
            for s in &sessions {
                let sid = if s.session_id.len() > 8 {
                    &s.session_id[..8]
                } else {
                    &s.session_id
                };
                let model = if s.model.len() > 26 {
                    format!("{}…", &s.model[..25])
                } else {
                    s.model.clone()
                };
                if show_all {
                    println!(
                        "{:<10} {:<28} {:>5} {:>7} {:<20} {}",
                        sid, model, s.turn_count, s.client_count, s.last_active, s.socket_path
                    );
                } else {
                    println!(
                        "{:<10} {:<28} {:>5} {:>7} {}",
                        sid, model, s.turn_count, s.client_count, s.last_active
                    );
                }
            }
            println!("{} session(s)", sessions.len());
        }
        ControlResponse::Error { message } => {
            eprintln!("Error: {message}");
        }
        other => {
            eprintln!("Unexpected response: {other:?}");
        }
    }
    Ok(())
}

// ── Create / Kill ───────────────────────────────────────────────────────────

async fn create(model: Option<String>, system_prompt: Option<String>) -> Result<()> {
    let resp = send_control(ControlCommand::CreateSession {
        model,
        system_prompt,
        token: None,
        resume_id: None,
        continue_last: false,
        cwd: None,
    })
    .await?;
    match resp {
        ControlResponse::Created {
            session_id,
            socket_path,
        } => {
            println!("Created session: {session_id}");
            println!("  Socket: {socket_path}");
            println!("  Attach: clankers attach {session_id}");
        }
        ControlResponse::Error { message } => {
            eprintln!("Error: {message}");
        }
        other => {
            eprintln!("Unexpected response: {other:?}");
        }
    }
    Ok(())
}

async fn kill(session_id: String) -> Result<()> {
    let resp = send_control(ControlCommand::KillSession { session_id }).await?;
    match resp {
        ControlResponse::Killed => println!("Session killed."),
        ControlResponse::Error { message } => eprintln!("Error: {message}"),
        other => eprintln!("Unexpected response: {other:?}"),
    }
    Ok(())
}

// ── Logs ────────────────────────────────────────────────────────────────────

fn logs(follow: bool, lines: usize) -> Result<()> {
    let log_path = transport::daemon_log_path();
    if !log_path.exists() {
        println!("No log file at {}", log_path.display());
        return Ok(());
    }

    let file =
        std::fs::File::open(&log_path).map_err(|e| crate::error::Error::Io { source: e })?;

    if follow {
        // tail -f: seek to end, print last N lines, then follow
        print_tail_lines(&file, lines)?;
        follow_file(file)?;
    } else {
        print_tail_lines(&file, lines)?;
    }
    Ok(())
}

/// Print the last N lines of a file.
fn print_tail_lines(file: &std::fs::File, n: usize) -> Result<()> {
    let reader = std::io::BufReader::new(file);
    let all_lines: Vec<String> = reader.lines().collect::<std::io::Result<_>>().map_err(|e| {
        crate::error::Error::Io { source: e }
    })?;
    let start = all_lines.len().saturating_sub(n);
    for line in &all_lines[start..] {
        println!("{line}");
    }
    Ok(())
}

/// Follow a file, printing new lines as they appear (like `tail -f`).
fn follow_file(mut file: std::fs::File) -> Result<()> {
    // Seek to end
    file.seek(std::io::SeekFrom::End(0))
        .map_err(|e| crate::error::Error::Io { source: e })?;

    let mut reader = std::io::BufReader::new(file);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // No new data — sleep briefly and retry
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            Ok(_) => {
                print!("{line}");
            }
            Err(e) => {
                return Err(crate::error::Error::Io { source: e });
            }
        }
    }
}

// ── Merge daemon ────────────────────────────────────────────────────────────

/// Run the merge daemon (watches for completed workers and auto-merges).
pub async fn run_merge_daemon(ctx: &CommandContext, interval: u64, once: bool) -> Result<()> {
    let repo_root = std::path::PathBuf::from(&ctx.cwd);

    let provider = crate::provider::discovery::build_router(
        ctx.api_key.as_deref(),
        ctx.api_base.clone(),
        &ctx.paths.global_auth,
        ctx.paths.pi_auth.as_deref(),
        None,
    )
    .ok();

    let db_path = ctx.paths.global_config_dir.join("clankers.db");
    let db = crate::db::Db::open(&db_path).map_err(|e| crate::error::Error::Io {
        source: std::io::Error::other(format!("failed to open database: {}", e)),
    })?;
    crate::worktree::merge_daemon::run_polling(db, repo_root, interval, once, provider, ctx.model.clone()).await;
    Ok(())
}

// ── Control socket helper ───────────────────────────────────────────────────

/// Send a control command to the daemon and return the response.
async fn send_control(cmd: ControlCommand) -> Result<ControlResponse> {
    let path = transport::control_socket_path();
    let stream = UnixStream::connect(&path).await.map_err(|e| {
        crate::error::Error::Provider {
            message: format!(
                "Cannot connect to daemon at {}: {e}\nIs the daemon running? Start with: clankers daemon start",
                path.display()
            ),
        }
    })?;

    let (mut reader, mut writer) = stream.into_split();

    frame::write_frame(&mut writer, &cmd)
        .await
        .map_err(|e| crate::error::Error::Provider {
            message: format!("Failed to send command: {e}"),
        })?;

    let resp: ControlResponse =
        frame::read_frame(&mut reader)
            .await
            .map_err(|e| crate::error::Error::Provider {
                message: format!("Failed to read response: {e}"),
            })?;

    Ok(resp)
}

fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}
