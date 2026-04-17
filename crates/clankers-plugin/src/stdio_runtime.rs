use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::PluginManager;
use crate::PluginState;
use crate::manifest::PluginKind;
use crate::manifest::PluginWorkingDirectory;
use crate::stdio_protocol::HostToPluginFrame;
use crate::stdio_protocol::PluginRuntimeMode;
use crate::stdio_protocol::PluginToHostFrame;
use crate::stdio_protocol::RegisteredTool;
use crate::stdio_protocol::STDIO_PLUGIN_PROTOCOL_VERSION;
use crate::stdio_protocol::read_plugin_to_host_frame;
use crate::stdio_protocol::write_stdio_plugin_frame;

const DEFAULT_RESTART_DELAYS_SECS: [u64; 5] = [1, 2, 4, 8, 16];
const SHUTDOWN_GRACE_SECS: u64 = 5;
const STDERR_TAIL_LINES: usize = 20;
const RUNTIME_SHUTDOWN_WAIT_MS: u64 = 250;
const RUNTIME_SHUTDOWN_POLL_MS: u64 = 25;

#[derive(Debug, Clone)]
pub(crate) struct StdioBootstrapConfig {
    pub cwd: PathBuf,
    pub mode: PluginRuntimeMode,
}

#[derive(Clone)]
pub(crate) struct StdioSupervisorHandle {
    command_tx: mpsc::UnboundedSender<SupervisorCommand>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StdioLiveState {
    pub tools: Vec<RegisteredTool>,
    pub event_subscriptions: Vec<String>,
    pub stderr_tail: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum ShutdownTargetState {
    Loaded,
    Disabled,
}

enum SupervisorCommand {
    Shutdown {
        reason: String,
        target_state: ShutdownTargetState,
    },
}

enum SupervisorEvent {
    Frame(PluginToHostFrame),
    ReaderClosed,
    ReadError(String),
    StderrLine(String),
}

enum ConnectionOutcome {
    Stopped {
        target_state: ShutdownTargetState,
    },
    UnexpectedExit {
        ready_seen: bool,
        reason: String,
    },
}

pub fn configure_stdio_runtime(
    manager: &Arc<Mutex<PluginManager>>,
    cwd: PathBuf,
    mode: PluginRuntimeMode,
) {
    let mut manager = manager.lock().unwrap_or_else(|poisoned| {
        warn!("Plugin manager mutex was poisoned while configuring stdio runtime, recovering");
        poisoned.into_inner()
    });
    manager.stdio_bootstrap = Some(StdioBootstrapConfig { cwd, mode });
}

pub fn start_stdio_plugins(manager: &Arc<Mutex<PluginManager>>) {
    if tokio::runtime::Handle::try_current().is_err() {
        debug!("no active tokio runtime; leaving stdio plugins discovered but not launched");
        return;
    }

    let names = {
        let manager = manager.lock().unwrap_or_else(|poisoned| {
            warn!("Plugin manager mutex was poisoned while collecting stdio plugins, recovering");
            poisoned.into_inner()
        });
        manager
            .plugins
            .iter()
            .filter(|(_, info)| {
                info.manifest.kind == PluginKind::Stdio
                    && !matches!(info.state, PluginState::Disabled | PluginState::Error(_))
            })
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>()
    };

    for name in names {
        if let Err(error) = start_stdio_plugin(manager, &name) {
            warn!("Failed to start stdio plugin '{}': {}", name, error);
            set_plugin_state(manager, &name, PluginState::Error(error));
        }
    }
}

pub async fn shutdown_plugin_runtime(manager: &Arc<Mutex<PluginManager>>, reason: &str) {
    let handles = {
        let manager = manager.lock().unwrap_or_else(|poisoned| {
            warn!("Plugin manager mutex was poisoned while shutting down stdio runtime, recovering");
            poisoned.into_inner()
        });
        manager
            .stdio_supervisors
            .iter()
            .map(|(name, handle)| (name.clone(), handle.clone()))
            .collect::<Vec<_>>()
    };

    for (name, handle) in &handles {
        if handle
            .command_tx
            .send(SupervisorCommand::Shutdown {
                reason: reason.to_string(),
                target_state: ShutdownTargetState::Loaded,
            })
            .is_err()
        {
            debug!("stdio supervisor '{}' already stopped during shutdown", name);
        }
    }

    let deadline = tokio::time::Instant::now() + Duration::from_millis(RUNTIME_SHUTDOWN_WAIT_MS);
    loop {
        let remaining = {
            let manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            manager.stdio_supervisors.len()
        };
        if remaining == 0 || tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(RUNTIME_SHUTDOWN_POLL_MS)).await;
    }
}

pub(crate) fn start_stdio_plugin(manager: &Arc<Mutex<PluginManager>>, name: &str) -> Result<(), String> {
    let handle = tokio::runtime::Handle::try_current().map_err(|_| {
        format!(
            "cannot launch stdio plugin '{}' without an active tokio runtime",
            name
        )
    })?;

    let (bootstrap, should_start) = {
        let mut manager = manager.lock().unwrap_or_else(|poisoned| {
            warn!("Plugin manager mutex was poisoned while starting stdio plugin, recovering");
            poisoned.into_inner()
        });
        let Some(info) = manager.plugins.get(name) else {
            return Err(format!("Plugin '{}' not found", name));
        };
        if info.manifest.kind != PluginKind::Stdio {
            return Err(format!("Plugin '{}' is not a stdio plugin", name));
        }
        if matches!(info.state, PluginState::Disabled | PluginState::Error(_)) {
            return Ok(());
        }
        if manager.stdio_supervisors.contains_key(name) {
            return Ok(());
        }
        let bootstrap = manager
            .stdio_bootstrap
            .clone()
            .ok_or_else(|| format!("stdio runtime not configured for plugin '{}'", name))?;
        manager.plugins.get_mut(name).expect("checked above").state = PluginState::Starting;
        manager.stdio_live_state.remove(name);
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        manager
            .stdio_supervisors
            .insert(name.to_string(), StdioSupervisorHandle { command_tx });
        (bootstrap, command_rx)
    };

    let manager = Arc::clone(manager);
    let name = name.to_string();
    handle.spawn(async move {
        supervise_stdio_plugin(manager, name, bootstrap, should_start).await;
    });
    Ok(())
}

pub(crate) fn start_stdio_plugin_from_manager(
    _manager: &mut PluginManager,
    name: &str,
) -> Result<(), String> {
    Err(format!(
        "stdio plugin '{}' can only be started from initialized runtime startup paths",
        name
    ))
}

pub(crate) fn stop_stdio_plugin(
    manager: &mut PluginManager,
    name: &str,
    reason: &str,
    target_state: PluginState,
) {
    let Some(handle) = manager.stdio_supervisors.remove(name) else {
        return;
    };
    manager.stdio_live_state.remove(name);
    if let Some(info) = manager.plugins.get_mut(name) {
        info.state = target_state.clone();
    }
    let target_state = match target_state {
        PluginState::Disabled => ShutdownTargetState::Disabled,
        _ => ShutdownTargetState::Loaded,
    };
    handle
        .command_tx
        .send(SupervisorCommand::Shutdown {
            reason: reason.to_string(),
            target_state,
        })
        .ok();
}

pub(crate) fn live_tools(manager: &PluginManager, name: &str) -> Vec<String> {
    manager
        .stdio_live_state
        .get(name)
        .map(|state| state.tools.iter().map(|tool| tool.name.clone()).collect())
        .unwrap_or_default()
}

pub(crate) fn live_event_subscriptions(manager: &PluginManager, name: &str) -> Vec<String> {
    manager
        .stdio_live_state
        .get(name)
        .map(|state| state.event_subscriptions.clone())
        .unwrap_or_default()
}

async fn supervise_stdio_plugin(
    manager: Arc<Mutex<PluginManager>>,
    name: String,
    bootstrap: StdioBootstrapConfig,
    mut command_rx: mpsc::UnboundedReceiver<SupervisorCommand>,
) {
    let restart_delays = restart_delays();
    let mut failed_starts_without_ready = 0usize;

    loop {
        set_plugin_state(&manager, &name, PluginState::Starting);
        clear_live_state(&manager, &name);

        let outcome = match run_stdio_connection(
            Arc::clone(&manager),
            &name,
            &bootstrap,
            &mut command_rx,
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(error) => ConnectionOutcome::UnexpectedExit {
                ready_seen: false,
                reason: error,
            },
        };

        match outcome {
            ConnectionOutcome::Stopped { target_state } => {
                remove_supervisor(&manager, &name);
                clear_live_state(&manager, &name);
                match target_state {
                    ShutdownTargetState::Loaded => {
                        set_plugin_state(&manager, &name, PluginState::Loaded);
                    }
                    ShutdownTargetState::Disabled => {
                        set_plugin_state(&manager, &name, PluginState::Disabled);
                    }
                }
                return;
            }
            ConnectionOutcome::UnexpectedExit { ready_seen, reason } => {
                clear_live_state(&manager, &name);
                if ready_seen {
                    failed_starts_without_ready = 0;
                } else {
                    failed_starts_without_ready += 1;
                }

                if !ready_seen && failed_starts_without_ready >= restart_delays.len() {
                    remove_supervisor(&manager, &name);
                    set_plugin_state(&manager, &name, PluginState::Error(reason));
                    return;
                }

                let delay = if ready_seen {
                    restart_delays[0]
                } else {
                    restart_delays[failed_starts_without_ready.saturating_sub(1)]
                };
                set_plugin_state(&manager, &name, PluginState::Backoff(reason));

                tokio::select! {
                    biased;
                    command = command_rx.recv() => {
                        if let Some(SupervisorCommand::Shutdown { target_state, .. }) = command {
                            remove_supervisor(&manager, &name);
                            clear_live_state(&manager, &name);
                            match target_state {
                                ShutdownTargetState::Loaded => set_plugin_state(&manager, &name, PluginState::Loaded),
                                ShutdownTargetState::Disabled => set_plugin_state(&manager, &name, PluginState::Disabled),
                            }
                            return;
                        }
                        remove_supervisor(&manager, &name);
                        clear_live_state(&manager, &name);
                        set_plugin_state(&manager, &name, PluginState::Loaded);
                        return;
                    }
                    _ = tokio::time::sleep(delay) => {}
                }
            }
        }
    }
}

async fn run_stdio_connection(
    manager: Arc<Mutex<PluginManager>>,
    name: &str,
    bootstrap: &StdioBootstrapConfig,
    command_rx: &mut mpsc::UnboundedReceiver<SupervisorCommand>,
) -> Result<ConnectionOutcome, String> {
    let launch = collect_launch_spec(&manager, name, bootstrap)?;

    let mut command = Command::new(&launch.command);
    command
        .args(&launch.args)
        .current_dir(&launch.cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = command.spawn().map_err(|error| format!("failed to spawn '{}': {}", name, error))?;
    let child_pid = child.id();
    info!(plugin = %name, pid = ?child_pid, "started stdio plugin");

    let stdin = child.stdin.take().ok_or_else(|| format!("stdio plugin '{}' missing stdin pipe", name))?;
    let stdout = child.stdout.take().ok_or_else(|| format!("stdio plugin '{}' missing stdout pipe", name))?;
    let stderr = child.stderr.take().ok_or_else(|| format!("stdio plugin '{}' missing stderr pipe", name))?;

    let (writer_tx, mut writer_rx) = mpsc::unbounded_channel::<HostToPluginFrame>();
    tokio::spawn(async move {
        let mut stdin = stdin;
        while let Some(frame) = writer_rx.recv().await {
            if write_stdio_plugin_frame(&mut stdin, &frame).await.is_err() {
                break;
            }
        }
    });

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<SupervisorEvent>();
    let reader_tx = event_tx.clone();
    tokio::spawn(async move {
        let mut stdout = stdout;
        loop {
            match read_plugin_to_host_frame(&mut stdout).await {
                Ok(frame) => {
                    if reader_tx.send(SupervisorEvent::Frame(frame)).is_err() {
                        break;
                    }
                }
                Err(crate::stdio_protocol::StdioProtocolError::Io(error))
                    if error.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    reader_tx.send(SupervisorEvent::ReaderClosed).ok();
                    break;
                }
                Err(error) => {
                    reader_tx.send(SupervisorEvent::ReadError(error.to_string())).ok();
                    break;
                }
            }
        }
    });

    let stderr_tx = event_tx;
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if stderr_tx.send(SupervisorEvent::StderrLine(line)).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    stderr_tx
                        .send(SupervisorEvent::StderrLine(format!("stderr read error: {}", error)))
                        .ok();
                    break;
                }
            }
        }
    });

    writer_tx
        .send(HostToPluginFrame::Hello {
            plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
            plugin: name.to_string(),
            cwd: bootstrap.cwd.display().to_string(),
            mode: bootstrap.mode.clone(),
        })
        .map_err(|_| format!("stdio plugin '{}' failed before host hello could be sent", name))?;

    let mut hello_seen = false;
    let mut ready_seen = false;
    let mut pending_disconnect_reason: Option<String> = None;

    loop {
        tokio::select! {
            biased;
            command = command_rx.recv() => {
                match command {
                    Some(SupervisorCommand::Shutdown { reason, target_state }) => {
                        writer_tx.send(HostToPluginFrame::Shutdown {
                            plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                            reason,
                        }).ok();
                        match tokio::time::timeout(Duration::from_secs(SHUTDOWN_GRACE_SECS), child.wait()).await {
                            Ok(Ok(_)) | Ok(Err(_)) => {}
                            Err(_) => {
                                child.start_kill().ok();
                                child.wait().await.ok();
                            }
                        }
                        return Ok(ConnectionOutcome::Stopped { target_state });
                    }
                    None => {
                        return Ok(ConnectionOutcome::Stopped { target_state: ShutdownTargetState::Loaded });
                    }
                }
            }
            Some(event) = event_rx.recv() => {
                match event {
                    SupervisorEvent::Frame(frame) => {
                        if !ready_seen {
                            handle_startup_frame(&manager, name, &mut hello_seen, &mut ready_seen, frame)?;
                        } else {
                            handle_runtime_frame(&manager, name, frame)?;
                        }
                    }
                    SupervisorEvent::ReaderClosed => {
                        pending_disconnect_reason.get_or_insert_with(|| "plugin closed stdio connection".to_string());
                    }
                    SupervisorEvent::ReadError(error) => {
                        pending_disconnect_reason = Some(error);
                    }
                    SupervisorEvent::StderrLine(line) => {
                        record_stderr_line(&manager, name, &line);
                    }
                }
            }
            status = child.wait() => {
                let reason = match status {
                    Ok(status) if status.success() => {
                        pending_disconnect_reason.unwrap_or_else(|| {
                            if ready_seen {
                                format!("stdio plugin '{}' exited unexpectedly", name)
                            } else {
                                format!("stdio plugin '{}' exited before ready", name)
                            }
                        })
                    }
                    Ok(status) => {
                        let base = format!("stdio plugin '{}' exited with status {}", name, status);
                        enrich_reason_with_stderr(&manager, name, base)
                    }
                    Err(error) => {
                        enrich_reason_with_stderr(&manager, name, format!("failed waiting for stdio plugin '{}': {}", name, error))
                    }
                };
                return Ok(ConnectionOutcome::UnexpectedExit {
                    ready_seen,
                    reason,
                });
            }
        }
    }
}

struct LaunchSpec {
    command: String,
    args: Vec<String>,
    cwd: PathBuf,
}

fn collect_launch_spec(
    manager: &Arc<Mutex<PluginManager>>,
    name: &str,
    bootstrap: &StdioBootstrapConfig,
) -> Result<LaunchSpec, String> {
    let manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let info = manager
        .plugins
        .get(name)
        .ok_or_else(|| format!("Plugin '{}' not found", name))?;
    let stdio = info
        .manifest
        .stdio
        .as_ref()
        .ok_or_else(|| format!("Plugin '{}' missing stdio launch policy", name))?;
    let command = stdio
        .command
        .clone()
        .ok_or_else(|| format!("Plugin '{}' missing stdio command", name))?;
    let cwd = match stdio.working_dir {
        Some(PluginWorkingDirectory::PluginDir) => info.path.clone(),
        Some(PluginWorkingDirectory::ProjectRoot) | None => bootstrap.cwd.clone(),
    };

    Ok(LaunchSpec {
        command,
        args: stdio.args.clone(),
        cwd,
    })
}

fn handle_startup_frame(
    manager: &Arc<Mutex<PluginManager>>,
    name: &str,
    hello_seen: &mut bool,
    ready_seen: &mut bool,
    frame: PluginToHostFrame,
) -> Result<(), String> {
    match frame {
        PluginToHostFrame::Hello { plugin, .. } => {
            if *hello_seen {
                return Err(enrich_reason_with_stderr(
                    manager,
                    name,
                    format!("stdio plugin '{}' sent duplicate hello frame", name),
                ));
            }
            if plugin != name {
                return Err(enrich_reason_with_stderr(
                    manager,
                    name,
                    format!("stdio plugin '{}' identified as '{}' during handshake", name, plugin),
                ));
            }
            *hello_seen = true;
            Ok(())
        }
        PluginToHostFrame::Ready { .. } => {
            if !*hello_seen {
                return Err(enrich_reason_with_stderr(
                    manager,
                    name,
                    format!("stdio plugin '{}' sent ready before hello", name),
                ));
            }
            *ready_seen = true;
            set_plugin_state(manager, name, PluginState::Active);
            Ok(())
        }
        other => Err(enrich_reason_with_stderr(
            manager,
            name,
            format!("stdio plugin '{}' sent {:?} before ready", name, frame_kind(&other)),
        )),
    }
}

fn handle_runtime_frame(
    manager: &Arc<Mutex<PluginManager>>,
    name: &str,
    frame: PluginToHostFrame,
) -> Result<(), String> {
    match frame {
        PluginToHostFrame::RegisterTools { tools, .. } => {
            let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let state = manager.stdio_live_state.entry(name.to_string()).or_default();
            for tool in tools {
                if !state.tools.iter().any(|existing| existing.name == tool.name) {
                    state.tools.push(tool);
                }
            }
            Ok(())
        }
        PluginToHostFrame::UnregisterTools { tools, .. } => {
            let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let state = manager.stdio_live_state.entry(name.to_string()).or_default();
            state.tools.retain(|tool| !tools.iter().any(|name| name == &tool.name));
            Ok(())
        }
        PluginToHostFrame::SubscribeEvents { events, .. } => {
            let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let state = manager.stdio_live_state.entry(name.to_string()).or_default();
            state.event_subscriptions = events;
            Ok(())
        }
        PluginToHostFrame::Ui { .. }
        | PluginToHostFrame::Display { .. }
        | PluginToHostFrame::ToolProgress { .. }
        | PluginToHostFrame::ToolResult { .. }
        | PluginToHostFrame::ToolError { .. }
        | PluginToHostFrame::ToolCancelled { .. } => Ok(()),
        PluginToHostFrame::Hello { .. } | PluginToHostFrame::Ready { .. } => Err(enrich_reason_with_stderr(
            manager,
            name,
            format!("stdio plugin '{}' sent duplicate startup frame after ready", name),
        )),
    }
}

fn frame_kind(frame: &PluginToHostFrame) -> &'static str {
    match frame {
        PluginToHostFrame::Hello { .. } => "hello",
        PluginToHostFrame::Ready { .. } => "ready",
        PluginToHostFrame::RegisterTools { .. } => "register_tools",
        PluginToHostFrame::UnregisterTools { .. } => "unregister_tools",
        PluginToHostFrame::SubscribeEvents { .. } => "subscribe_events",
        PluginToHostFrame::ToolProgress { .. } => "tool_progress",
        PluginToHostFrame::ToolResult { .. } => "tool_result",
        PluginToHostFrame::ToolError { .. } => "tool_error",
        PluginToHostFrame::ToolCancelled { .. } => "tool_cancelled",
        PluginToHostFrame::Ui { .. } => "ui",
        PluginToHostFrame::Display { .. } => "display",
    }
}

fn record_stderr_line(manager: &Arc<Mutex<PluginManager>>, name: &str, line: &str) {
    debug!(plugin = %name, stderr = %line, "stdio plugin stderr");
    let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let state = manager.stdio_live_state.entry(name.to_string()).or_default();
    let mut stderr_tail: VecDeque<String> = state.stderr_tail.drain(..).collect();
    stderr_tail.push_back(line.to_string());
    while stderr_tail.len() > STDERR_TAIL_LINES {
        stderr_tail.pop_front();
    }
    state.stderr_tail = stderr_tail.into_iter().collect();
}

fn enrich_reason_with_stderr(manager: &Arc<Mutex<PluginManager>>, name: &str, base: String) -> String {
    let manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let stderr_tail = manager
        .stdio_live_state
        .get(name)
        .map(|state| state.stderr_tail.clone())
        .unwrap_or_default();
    if stderr_tail.is_empty() {
        base
    } else {
        format!("{} (stderr: {})", base, stderr_tail.join(" | "))
    }
}

fn set_plugin_state(manager: &Arc<Mutex<PluginManager>>, name: &str, state: PluginState) {
    let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(info) = manager.plugins.get_mut(name) {
        info.state = state;
    }
}

fn clear_live_state(manager: &Arc<Mutex<PluginManager>>, name: &str) {
    let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    manager.stdio_live_state.remove(name);
}

fn remove_supervisor(manager: &Arc<Mutex<PluginManager>>, name: &str) {
    let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    manager.stdio_supervisors.remove(name);
}

fn restart_delays() -> Vec<Duration> {
    if let Ok(value) = std::env::var("CLANKERS_STDIO_RESTART_DELAYS_MS") {
        let parsed: Vec<Duration> = value
            .split(',')
            .filter_map(|entry| entry.trim().parse::<u64>().ok())
            .map(Duration::from_millis)
            .collect();
        if !parsed.is_empty() {
            return parsed;
        }
    }

    DEFAULT_RESTART_DELAYS_SECS
        .into_iter()
        .map(Duration::from_secs)
        .collect()
}
