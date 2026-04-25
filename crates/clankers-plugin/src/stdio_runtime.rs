#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::collections::HashMap;
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
const DEFAULT_SHUTDOWN_GRACE_SECS: u64 = 5;
const RUNTIME_SHUTDOWN_EXTRA_SECS: u64 = 1;
const STDERR_TAIL_LINES: usize = 20;
const RUNTIME_SHUTDOWN_POLL_MS: u64 = 25;

#[derive(Debug, Clone)]
pub(crate) struct StdioBootstrapConfig {
    pub cwd: PathBuf,
    pub mode: PluginRuntimeMode,
}

#[derive(Clone)]
pub(crate) struct StdioSupervisorHandle {
    run_id: u64,
    command_tx: mpsc::UnboundedSender<SupervisorCommand>,
}

#[derive(Debug, Default)]
pub(crate) struct StdioLiveState {
    pub run_id: u64,
    pub tools: Vec<RegisteredTool>,
    pub event_subscriptions: Vec<String>,
    pub stderr_tail: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum StdioHostEvent {
    Ui(crate::ui::PluginUiAction),
    Display { plugin: String, message: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum StdioToolCallEvent {
    Progress(String),
    Result(serde_json::Value),
    Error(String),
    Cancelled,
    Disconnected(String),
}

#[derive(Debug, Clone, Copy)]
enum ShutdownTargetState {
    Loaded,
    Disabled,
}

struct PendingToolCall {
    event_tx: mpsc::UnboundedSender<StdioToolCallEvent>,
}

enum SupervisorCommand {
    DeliverEvent {
        event_name: String,
        data: serde_json::Value,
    },
    InvokeTool {
        call_id: String,
        tool: String,
        args: serde_json::Value,
        event_tx: mpsc::UnboundedSender<StdioToolCallEvent>,
    },
    CancelTool {
        call_id: String,
        reason: String,
    },
    DropToolCall {
        call_id: String,
    },
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
    Stopped { target_state: ShutdownTargetState },
    UnexpectedExit { ready_seen: bool, reason: String },
}

pub fn configure_stdio_runtime(manager: &Arc<Mutex<PluginManager>>, cwd: PathBuf, mode: PluginRuntimeMode) {
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

    let deadline = tokio::time::Instant::now() + runtime_shutdown_wait_duration();
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
    let handle = tokio::runtime::Handle::try_current()
        .map_err(|_| format!("cannot launch stdio plugin '{}' without an active tokio runtime", name))?;

    let restart_delays = restart_delays();

    let (bootstrap, run_id, should_start) = {
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
        let run_id = manager.next_stdio_run_id;
        manager.next_stdio_run_id += 1;
        manager.plugins.get_mut(name).expect("checked above").state = PluginState::Starting;
        manager.stdio_live_state.remove(name);
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        manager.stdio_supervisors.insert(name.to_string(), StdioSupervisorHandle { run_id, command_tx });
        (bootstrap, run_id, command_rx)
    };

    let manager = Arc::clone(manager);
    let name = name.to_string();
    handle.spawn(async move {
        supervise_stdio_plugin(manager, name, bootstrap, run_id, restart_delays, should_start).await;
    });
    Ok(())
}

pub(crate) fn stop_stdio_plugin(manager: &mut PluginManager, name: &str, reason: &str, target_state: PluginState) {
    let Some(handle) = manager.stdio_supervisors.remove(name) else {
        return;
    };
    if manager.stdio_live_state.get(name).is_some_and(|state| state.run_id == handle.run_id) {
        manager.stdio_live_state.remove(name);
    }
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

pub(crate) fn live_registered_tools(manager: &PluginManager, name: &str) -> Vec<RegisteredTool> {
    manager.stdio_live_state.get(name).map(|state| state.tools.clone()).unwrap_or_default()
}

pub(crate) fn live_event_subscriptions(manager: &PluginManager, name: &str) -> Vec<String> {
    manager
        .stdio_live_state
        .get(name)
        .map(|state| state.event_subscriptions.clone())
        .unwrap_or_default()
}

pub fn send_stdio_event(
    manager: &Arc<Mutex<PluginManager>>,
    plugin: &str,
    event_name: &str,
    data: serde_json::Value,
) -> Result<(), String> {
    let handle = {
        let manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let info = manager.plugins.get(plugin).ok_or_else(|| format!("Plugin '{}' not found", plugin))?;
        if info.manifest.kind != PluginKind::Stdio {
            return Err(format!("Plugin '{}' is not a stdio plugin", plugin));
        }
        if info.state != PluginState::Active {
            return Err(format!("Plugin '{}' is not active (state: {:?})", plugin, info.state));
        }
        manager
            .stdio_supervisors
            .get(plugin)
            .cloned()
            .ok_or_else(|| format!("Plugin '{}' is not connected", plugin))?
    };

    handle
        .command_tx
        .send(SupervisorCommand::DeliverEvent {
            event_name: event_name.to_string(),
            data,
        })
        .map_err(|_| format!("Plugin '{}' supervisor is not running", plugin))
}

pub fn drain_stdio_host_events(manager: &Arc<Mutex<PluginManager>>) -> Vec<StdioHostEvent> {
    let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::take(&mut manager.stdio_host_events)
}

pub fn start_stdio_tool_call(
    manager: &Arc<Mutex<PluginManager>>,
    plugin: &str,
    call_id: &str,
    tool: &str,
    args: serde_json::Value,
) -> Result<mpsc::UnboundedReceiver<StdioToolCallEvent>, String> {
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let handle = {
        let manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let info = manager.plugins.get(plugin).ok_or_else(|| format!("Plugin '{}' not found", plugin))?;
        if info.manifest.kind != PluginKind::Stdio {
            return Err(format!("Plugin '{}' is not a stdio plugin", plugin));
        }
        if info.state != PluginState::Active {
            return Err(format!("Plugin '{}' is not active (state: {:?})", plugin, info.state));
        }
        let tools = manager.stdio_live_state.get(plugin).map(|state| state.tools.as_slice()).unwrap_or(&[]);
        if !tools.iter().any(|registered| registered.name == tool) {
            return Err(format!("Plugin '{}' has not registered tool '{}'", plugin, tool));
        }
        manager
            .stdio_supervisors
            .get(plugin)
            .cloned()
            .ok_or_else(|| format!("Plugin '{}' is not connected", plugin))?
    };

    handle
        .command_tx
        .send(SupervisorCommand::InvokeTool {
            call_id: call_id.to_string(),
            tool: tool.to_string(),
            args,
            event_tx,
        })
        .map_err(|_| format!("Plugin '{}' supervisor is not running", plugin))?;
    Ok(event_rx)
}

pub fn cancel_stdio_tool_call(
    manager: &Arc<Mutex<PluginManager>>,
    plugin: &str,
    call_id: &str,
    reason: &str,
) -> Result<(), String> {
    let handle = {
        let manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        manager
            .stdio_supervisors
            .get(plugin)
            .cloned()
            .ok_or_else(|| format!("Plugin '{}' is not connected", plugin))?
    };
    handle
        .command_tx
        .send(SupervisorCommand::CancelTool {
            call_id: call_id.to_string(),
            reason: reason.to_string(),
        })
        .map_err(|_| format!("Plugin '{}' supervisor is not running", plugin))
}

pub fn abandon_stdio_tool_call(manager: &Arc<Mutex<PluginManager>>, plugin: &str, call_id: &str) -> Result<(), String> {
    let handle = {
        let manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        manager
            .stdio_supervisors
            .get(plugin)
            .cloned()
            .ok_or_else(|| format!("Plugin '{}' is not connected", plugin))?
    };
    handle
        .command_tx
        .send(SupervisorCommand::DropToolCall {
            call_id: call_id.to_string(),
        })
        .map_err(|_| format!("Plugin '{}' supervisor is not running", plugin))
}

async fn supervise_stdio_plugin(
    manager: Arc<Mutex<PluginManager>>,
    name: String,
    bootstrap: StdioBootstrapConfig,
    run_id: u64,
    restart_delays: Vec<Duration>,
    mut command_rx: mpsc::UnboundedReceiver<SupervisorCommand>,
) {
    let mut failed_starts_without_ready = 0usize;

    loop {
        set_plugin_state_if_current(&manager, &name, run_id, PluginState::Starting);
        clear_live_state_if_run(&manager, &name, run_id);

        let outcome = match run_stdio_connection(Arc::clone(&manager), &name, run_id, &bootstrap, &mut command_rx).await
        {
            Ok(outcome) => outcome,
            Err(error) => ConnectionOutcome::UnexpectedExit {
                ready_seen: false,
                reason: error,
            },
        };

        match outcome {
            ConnectionOutcome::Stopped { target_state } => {
                remove_supervisor_if_current(&manager, &name, run_id);
                clear_live_state_if_run(&manager, &name, run_id);
                match target_state {
                    ShutdownTargetState::Loaded => {
                        set_plugin_state_if_current_or_absent(&manager, &name, run_id, PluginState::Loaded);
                    }
                    ShutdownTargetState::Disabled => {
                        set_plugin_state_if_current_or_absent(&manager, &name, run_id, PluginState::Disabled);
                    }
                }
                return;
            }
            ConnectionOutcome::UnexpectedExit { ready_seen, reason } => {
                clear_live_state_if_run(&manager, &name, run_id);
                if ready_seen {
                    failed_starts_without_ready = 0;
                } else {
                    failed_starts_without_ready += 1;
                }

                if !ready_seen && failed_starts_without_ready >= restart_delays.len() {
                    remove_supervisor_if_current(&manager, &name, run_id);
                    set_plugin_state_if_current_or_absent(&manager, &name, run_id, PluginState::Error(reason));
                    return;
                }

                let delay = if ready_seen {
                    restart_delays[0]
                } else {
                    restart_delays[failed_starts_without_ready.saturating_sub(1)]
                };
                set_plugin_state_if_current(&manager, &name, run_id, PluginState::Backoff(reason));

                tokio::select! {
                    biased;
                    command = command_rx.recv() => {
                        match command {
                            Some(SupervisorCommand::Shutdown { target_state, .. }) => {
                                remove_supervisor_if_current(&manager, &name, run_id);
                                clear_live_state_if_run(&manager, &name, run_id);
                                match target_state {
                                    ShutdownTargetState::Loaded => {
                                        set_plugin_state_if_current_or_absent(&manager, &name, run_id, PluginState::Loaded);
                                    }
                                    ShutdownTargetState::Disabled => {
                                        set_plugin_state_if_current_or_absent(&manager, &name, run_id, PluginState::Disabled);
                                    }
                                }
                                return;
                            }
                            Some(SupervisorCommand::DeliverEvent { .. }) => {}
                            Some(SupervisorCommand::InvokeTool { event_tx, tool, .. }) => {
                                event_tx.send(StdioToolCallEvent::Error(format!(
                                    "Plugin '{}' cannot invoke '{}' while restarting",
                                    name, tool
                                ))).ok();
                            }
                            Some(SupervisorCommand::CancelTool { .. } | SupervisorCommand::DropToolCall { .. }) => {}
                            None => {
                                remove_supervisor_if_current(&manager, &name, run_id);
                                clear_live_state_if_run(&manager, &name, run_id);
                                set_plugin_state_if_current_or_absent(&manager, &name, run_id, PluginState::Loaded);
                                return;
                            }
                        }
                    }
                    () = tokio::time::sleep(delay) => {}
                }
            }
        }
    }
}

async fn run_stdio_connection(
    manager: Arc<Mutex<PluginManager>>,
    name: &str,
    run_id: u64,
    bootstrap: &StdioBootstrapConfig,
    command_rx: &mut mpsc::UnboundedReceiver<SupervisorCommand>,
) -> Result<ConnectionOutcome, String> {
    let launch = collect_launch_spec(&manager, name, bootstrap)?;

    if let LaunchSandbox::Restricted(policy) = &launch.sandbox {
        crate::restricted_sandbox::prepare_restricted_paths(&policy.writable_roots)?;
    }

    let mut command = Command::new(&launch.command);
    command
        .args(&launch.args)
        .current_dir(&launch.cwd)
        .env_clear()
        .envs(launch.env.iter().cloned())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    #[cfg(unix)]
    if let LaunchSandbox::Restricted(policy) = &launch.sandbox {
        let read_roots = policy.read_roots.clone();
        let writable_roots = policy.writable_roots.clone();
        let allow_network = policy.allow_network;
        unsafe {
            command.pre_exec(move || {
                if let Err(error) = crate::restricted_sandbox::apply_restricted_sandbox_to_current(
                    &read_roots,
                    &writable_roots,
                    allow_network,
                ) {
                    let message = format!("restricted sandbox bootstrap failed: {error}\n");
                    let _ = libc::write(libc::STDERR_FILENO, message.as_ptr().cast(), message.len());
                    libc::_exit(126);
                }
                Ok(())
            });
        }
    }

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
                    stderr_tx.send(SupervisorEvent::StderrLine(format!("stderr read error: {}", error))).ok();
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
    let mut pending_calls: HashMap<String, PendingToolCall> = HashMap::new();

    loop {
        tokio::select! {
            biased;
            command = command_rx.recv() => {
                match command {
                    Some(SupervisorCommand::DeliverEvent { event_name, data }) => {
                        if !ready_seen {
                            continue;
                        }
                        writer_tx.send(HostToPluginFrame::Event {
                            plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                            event: crate::stdio_protocol::PluginEventEnvelope {
                                name: event_name,
                                data,
                            },
                        }).ok();
                    }
                    Some(SupervisorCommand::InvokeTool { call_id, tool, args, event_tx }) => {
                        if !ready_seen {
                            event_tx.send(StdioToolCallEvent::Error(format!(
                                "Plugin '{}' is not ready for tool '{}'",
                                name, tool,
                            ))).ok();
                            continue;
                        }
                        if writer_tx.send(HostToPluginFrame::ToolInvoke {
                            plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                            call_id: call_id.clone(),
                            tool,
                            args,
                        }).is_err() {
                            event_tx.send(StdioToolCallEvent::Error(format!(
                                "Plugin '{}' disconnected before tool invocation",
                                name,
                            ))).ok();
                            continue;
                        }
                        pending_calls.insert(call_id, PendingToolCall { event_tx });
                    }
                    Some(SupervisorCommand::CancelTool { call_id, reason }) => {
                        if pending_calls.contains_key(&call_id) {
                            writer_tx.send(HostToPluginFrame::ToolCancel {
                                plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                                call_id,
                                reason,
                            }).ok();
                        }
                    }
                    Some(SupervisorCommand::DropToolCall { call_id }) => {
                        pending_calls.remove(&call_id);
                    }
                    Some(SupervisorCommand::Shutdown { reason, target_state }) => {
                        writer_tx.send(HostToPluginFrame::Shutdown {
                            plugin_protocol: STDIO_PLUGIN_PROTOCOL_VERSION,
                            reason: reason.clone(),
                        }).ok();
                        match tokio::time::timeout(shutdown_grace_duration(), child.wait()).await {
                            Ok(Ok(_) | Err(_)) => {}
                            Err(_) => {
                                child.start_kill().ok();
                                child.wait().await.ok();
                            }
                        }
                        finish_pending_calls(
                            &mut pending_calls,
                            StdioToolCallEvent::Disconnected(format!("Plugin '{}' shut down: {}", name, reason)),
                        );
                        return Ok(ConnectionOutcome::Stopped { target_state });
                    }
                    None => {
                        finish_pending_calls(
                            &mut pending_calls,
                            StdioToolCallEvent::Disconnected(format!("Plugin '{}' supervisor stopped", name)),
                        );
                        return Ok(ConnectionOutcome::Stopped { target_state: ShutdownTargetState::Loaded });
                    }
                }
            }
            Some(event) = event_rx.recv() => {
                match event {
                    SupervisorEvent::Frame(frame) => {
                        if !ready_seen {
                            handle_startup_frame(&manager, name, run_id, &mut hello_seen, &mut ready_seen, frame)?;
                        } else {
                            handle_runtime_frame(&manager, name, run_id, &mut pending_calls, frame)?;
                        }
                    }
                    SupervisorEvent::ReaderClosed => {
                        pending_disconnect_reason.get_or_insert_with(|| "plugin closed stdio connection".to_string());
                    }
                    SupervisorEvent::ReadError(error) => {
                        pending_disconnect_reason = Some(error);
                    }
                    SupervisorEvent::StderrLine(line) => {
                        record_stderr_line(&manager, name, run_id, &line);
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
                finish_pending_calls(&mut pending_calls, StdioToolCallEvent::Disconnected(reason.clone()));
                return Ok(ConnectionOutcome::UnexpectedExit {
                    ready_seen,
                    reason,
                });
            }
        }
    }
}

enum LaunchSandbox {
    Inherit,
    Restricted(RestrictedSandboxPolicy),
}

struct LaunchSpec {
    command: String,
    args: Vec<String>,
    cwd: PathBuf,
    env: Vec<(String, String)>,
    sandbox: LaunchSandbox,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RestrictedSandboxPolicy {
    state_dir: PathBuf,
    writable_roots: Vec<PathBuf>,
    read_roots: Vec<PathBuf>,
    allow_network: bool,
}

fn collect_launch_spec(
    manager: &Arc<Mutex<PluginManager>>,
    name: &str,
    bootstrap: &StdioBootstrapConfig,
) -> Result<LaunchSpec, String> {
    let manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let info = manager.plugins.get(name).ok_or_else(|| format!("Plugin '{}' not found", name))?;
    let stdio = info
        .manifest
        .stdio
        .as_ref()
        .ok_or_else(|| format!("Plugin '{}' missing stdio launch policy", name))?;
    let cwd = match stdio.working_dir {
        Some(PluginWorkingDirectory::PluginDir) => info.path.clone(),
        Some(PluginWorkingDirectory::ProjectRoot) | None => bootstrap.cwd.clone(),
    };
    let command = resolve_command_path(
        &stdio.command.clone().ok_or_else(|| format!("Plugin '{}' missing stdio command", name))?,
        &cwd,
    )?;
    let sandbox_mode = stdio.sandbox.clone().ok_or_else(|| format!("Plugin '{}' missing stdio sandbox mode", name))?;

    if sandbox_mode == crate::manifest::PluginSandboxMode::Restricted
        && !crate::restricted_sandbox::restricted_sandbox_supported_platform()
    {
        let restricted = collect_restricted_policy(&manager, info, stdio, bootstrap, &command)?;
        debug!(
            plugin = %name,
            state_dir = %restricted.state_dir.display(),
            writable_roots = ?restricted.writable_roots,
            allow_network = restricted.allow_network,
            "restricted stdio sandbox requested on unsupported platform"
        );
        return Err(format!(
            "Plugin '{}' requested restricted sandbox mode, but restricted sandbox mode is unavailable on this host (state_dir={}, writable_roots={:?}, allow_network={})",
            name,
            restricted.state_dir.display(),
            restricted.writable_roots,
            restricted.allow_network,
        ));
    }

    let mut env = Vec::new();
    for var in &stdio.env_allowlist {
        let value = std::env::var(var)
            .map_err(|_| format!("Plugin '{}' missing required environment variable '{}'", name, var))?;
        if !env.iter().any(|(existing, _)| existing == var) {
            env.push((var.clone(), value));
        }
    }

    let sandbox = if sandbox_mode == crate::manifest::PluginSandboxMode::Restricted {
        let restricted = collect_restricted_policy(&manager, info, stdio, bootstrap, &command)?;
        debug!(
            plugin = %name,
            state_dir = %restricted.state_dir.display(),
            writable_roots = ?restricted.writable_roots,
            read_roots = ?restricted.read_roots,
            allow_network = restricted.allow_network,
            "applying restricted stdio sandbox"
        );
        LaunchSandbox::Restricted(restricted)
    } else {
        LaunchSandbox::Inherit
    };

    Ok(LaunchSpec {
        command,
        args: stdio.args.clone(),
        cwd,
        env,
        sandbox,
    })
}

fn collect_restricted_policy(
    manager: &PluginManager,
    info: &crate::PluginInfo,
    stdio: &crate::manifest::StdioManifest,
    bootstrap: &StdioBootstrapConfig,
    command: &str,
) -> Result<RestrictedSandboxPolicy, String> {
    let state_root = plugin_state_root(&manager.global_dir);
    let state_dir = state_root.join(&info.name);
    let mut writable_roots = vec![state_dir.clone()];
    for root in &stdio.writable_roots {
        let resolved = resolve_writable_root(root, &bootstrap.cwd)?;
        if !writable_roots.contains(&resolved) {
            writable_roots.push(resolved);
        }
    }

    let mut read_roots = vec![bootstrap.cwd.clone(), info.path.clone(), state_root.clone()];
    if let Some(parent) = manager.global_dir.parent() {
        push_unique_path(&mut read_roots, parent.to_path_buf());
    }
    if let Some(parent) = PathBuf::from(command).parent() {
        push_unique_path(&mut read_roots, parent.to_path_buf());
    }

    let allow_network = stdio.allow_network
        && crate::sandbox::has_permission(&info.manifest.permissions, crate::sandbox::Permission::Net);

    Ok(RestrictedSandboxPolicy {
        state_dir,
        writable_roots,
        read_roots,
        allow_network,
    })
}

fn plugin_state_root(global_dir: &std::path::Path) -> PathBuf {
    if global_dir.file_name().is_some_and(|name| name == "plugins")
        && let Some(parent) = global_dir.parent()
    {
        return parent.join("plugin-state");
    }
    global_dir.join("plugin-state")
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn resolve_writable_root(root: &str, project_root: &std::path::Path) -> Result<PathBuf, String> {
    let path = PathBuf::from(root);
    if path.as_os_str().is_empty() {
        return Err("restricted stdio writable roots cannot be empty".to_string());
    }
    if path.is_absolute() {
        return Err(format!("restricted stdio writable root '{}' must be project-root-relative", root));
    }
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir | std::path::Component::RootDir | std::path::Component::Prefix(_)
        )
    }) {
        return Err(format!("restricted stdio writable root '{}' must stay within the project root", root));
    }

    Ok(project_root.join(path))
}

fn resolve_command_path(command: &str, cwd: &std::path::Path) -> Result<String, String> {
    let path = PathBuf::from(command);
    if path.is_absolute() {
        return Ok(path.to_string_lossy().into_owned());
    }

    if path.components().count() > 1 {
        let candidate = cwd.join(path);
        return Ok(candidate.to_string_lossy().into_owned());
    }

    let Some(path_env) = std::env::var_os("PATH") else {
        return Err(format!("could not resolve stdio command '{}' because PATH is unset", command));
    };
    for dir in std::env::split_paths(&path_env) {
        let candidate = dir.join(command);
        if candidate.is_file() {
            return Ok(candidate.to_string_lossy().into_owned());
        }
    }

    Err(format!("could not resolve stdio command '{}' in PATH", command))
}

fn handle_startup_frame(
    manager: &Arc<Mutex<PluginManager>>,
    name: &str,
    run_id: u64,
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
            set_plugin_state_if_current(manager, name, run_id, PluginState::Active);
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
    run_id: u64,
    pending_calls: &mut HashMap<String, PendingToolCall>,
    frame: PluginToHostFrame,
) -> Result<(), String> {
    match frame {
        PluginToHostFrame::RegisterTools { tools, .. } => {
            let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let mut accepted = Vec::new();
            for tool in tools {
                let collision = tool_collision_owner(&manager, name, &tool.name);
                let state = live_state_for_run(&mut manager, name, run_id);
                if state.tools.iter().any(|existing| existing.name == tool.name)
                    || accepted.iter().any(|existing: &RegisteredTool| existing.name == tool.name)
                {
                    continue;
                }
                if let Some(owner) = collision {
                    warn!(plugin = %name, tool = %tool.name, owner = %owner, "rejecting colliding stdio tool registration");
                    continue;
                }
                accepted.push(tool);
            }
            let state = live_state_for_run(&mut manager, name, run_id);
            state.tools.extend(accepted);
            Ok(())
        }
        PluginToHostFrame::UnregisterTools { tools, .. } => {
            let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let state = live_state_for_run(&mut manager, name, run_id);
            state.tools.retain(|tool| !tools.iter().any(|name| name == &tool.name));
            Ok(())
        }
        PluginToHostFrame::SubscribeEvents { events, .. } => {
            let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let state = live_state_for_run(&mut manager, name, run_id);
            state.event_subscriptions = events;
            Ok(())
        }
        PluginToHostFrame::ToolProgress { call_id, message, .. } => {
            if let Some(call) = pending_calls.get(&call_id) {
                call.event_tx.send(StdioToolCallEvent::Progress(message)).ok();
            }
            Ok(())
        }
        PluginToHostFrame::ToolResult { call_id, content, .. } => {
            if let Some(call) = pending_calls.remove(&call_id) {
                call.event_tx.send(StdioToolCallEvent::Result(content)).ok();
            }
            Ok(())
        }
        PluginToHostFrame::ToolError { call_id, message, .. } => {
            if let Some(call) = pending_calls.remove(&call_id) {
                call.event_tx.send(StdioToolCallEvent::Error(message)).ok();
            }
            Ok(())
        }
        PluginToHostFrame::ToolCancelled { call_id, .. } => {
            if let Some(call) = pending_calls.remove(&call_id) {
                call.event_tx.send(StdioToolCallEvent::Cancelled).ok();
            }
            Ok(())
        }
        PluginToHostFrame::Ui { actions, .. } => {
            queue_ui_actions(manager, name, run_id, actions);
            Ok(())
        }
        PluginToHostFrame::Display { message, .. } => {
            queue_display_message(manager, name, run_id, message);
            Ok(())
        }
        PluginToHostFrame::Hello { .. } | PluginToHostFrame::Ready { .. } => Err(enrich_reason_with_stderr(
            manager,
            name,
            format!("stdio plugin '{}' sent duplicate startup frame after ready", name),
        )),
    }
}

fn finish_pending_calls(pending_calls: &mut HashMap<String, PendingToolCall>, event: StdioToolCallEvent) {
    for (_, call) in pending_calls.drain() {
        call.event_tx.send(event.clone()).ok();
    }
}

fn tool_collision_owner(manager: &PluginManager, plugin_name: &str, tool_name: &str) -> Option<String> {
    if manager.stdio_reserved_tool_names.contains(tool_name) {
        return Some("built-in".to_string());
    }

    for (other_plugin, info) in &manager.plugins {
        if other_plugin == plugin_name {
            continue;
        }
        if info.manifest.kind.uses_wasm_runtime()
            && info.state == PluginState::Active
            && info.declared_tool_inventory().iter().any(|existing| existing == tool_name)
        {
            return Some(other_plugin.clone());
        }
    }

    for (other_plugin, state) in &manager.stdio_live_state {
        if other_plugin == plugin_name {
            continue;
        }
        if state.tools.iter().any(|existing| existing.name == tool_name) {
            return Some(other_plugin.clone());
        }
    }

    None
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

fn queue_ui_actions(manager: &Arc<Mutex<PluginManager>>, name: &str, run_id: u64, actions: Vec<serde_json::Value>) {
    let parsed = crate::bridge::parse_ui_actions(name, &serde_json::json!({"ui": actions}));
    if parsed.is_empty() {
        return;
    }

    let permissions = {
        let manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        manager.plugins.get(name).map(|info| info.manifest.permissions.clone()).unwrap_or_default()
    };
    let filtered = crate::sandbox::filter_ui_actions(&permissions, parsed);
    if filtered.is_empty() {
        return;
    }

    let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if manager.stdio_supervisors.get(name).is_some_and(|handle| handle.run_id == run_id) {
        manager.stdio_host_events.extend(filtered.into_iter().map(StdioHostEvent::Ui));
    }
}

fn queue_display_message(manager: &Arc<Mutex<PluginManager>>, name: &str, run_id: u64, message: String) {
    if message.is_empty() {
        return;
    }

    let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if manager.stdio_supervisors.get(name).is_some_and(|handle| handle.run_id == run_id) {
        manager.stdio_host_events.push(StdioHostEvent::Display {
            plugin: name.to_string(),
            message,
        });
    }
}

fn record_stderr_line(manager: &Arc<Mutex<PluginManager>>, name: &str, run_id: u64, line: &str) {
    debug!(plugin = %name, stderr = %line, "stdio plugin stderr");
    let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let state = live_state_for_run(&mut manager, name, run_id);
    let mut stderr_tail: VecDeque<String> = state.stderr_tail.drain(..).collect();
    stderr_tail.push_back(line.to_string());
    while stderr_tail.len() > STDERR_TAIL_LINES {
        stderr_tail.pop_front();
    }
    state.stderr_tail = stderr_tail.into_iter().collect();
}

fn enrich_reason_with_stderr(manager: &Arc<Mutex<PluginManager>>, name: &str, base: String) -> String {
    let manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let stderr_tail = manager.stdio_live_state.get(name).map(|state| state.stderr_tail.clone()).unwrap_or_default();
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

fn set_plugin_state_if_current(manager: &Arc<Mutex<PluginManager>>, name: &str, run_id: u64, state: PluginState) {
    let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if manager.stdio_supervisors.get(name).is_some_and(|handle| handle.run_id == run_id)
        && let Some(info) = manager.plugins.get_mut(name)
    {
        info.state = state;
    }
}

fn set_plugin_state_if_current_or_absent(
    manager: &Arc<Mutex<PluginManager>>,
    name: &str,
    run_id: u64,
    state: PluginState,
) {
    let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let should_update = match manager.stdio_supervisors.get(name) {
        Some(handle) => handle.run_id == run_id,
        None => true,
    };
    if should_update && let Some(info) = manager.plugins.get_mut(name) {
        info.state = state;
    }
}

fn live_state_for_run<'a>(manager: &'a mut PluginManager, name: &str, run_id: u64) -> &'a mut StdioLiveState {
    let state = manager.stdio_live_state.entry(name.to_string()).or_default();
    if state.run_id != run_id {
        *state = StdioLiveState {
            run_id,
            ..Default::default()
        };
    }
    state
}

fn clear_live_state_if_run(manager: &Arc<Mutex<PluginManager>>, name: &str, run_id: u64) {
    let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if manager.stdio_live_state.get(name).is_some_and(|state| state.run_id == run_id) {
        manager.stdio_live_state.remove(name);
    }
}

fn remove_supervisor_if_current(manager: &Arc<Mutex<PluginManager>>, name: &str, run_id: u64) {
    let mut manager = manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if manager.stdio_supervisors.get(name).is_some_and(|handle| handle.run_id == run_id) {
        manager.stdio_supervisors.remove(name);
    }
}

fn shutdown_grace_duration() -> Duration {
    if let Ok(value) = std::env::var("CLANKERS_STDIO_SHUTDOWN_GRACE_MS")
        && let Ok(parsed) = value.trim().parse::<u64>()
    {
        return Duration::from_millis(parsed);
    }

    Duration::from_secs(DEFAULT_SHUTDOWN_GRACE_SECS)
}

fn runtime_shutdown_wait_duration() -> Duration {
    shutdown_grace_duration() + Duration::from_secs(RUNTIME_SHUTDOWN_EXTRA_SECS)
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

    DEFAULT_RESTART_DELAYS_SECS.into_iter().map(Duration::from_secs).collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::MutexGuard;
    use std::sync::OnceLock;

    use super::*;

    struct EnvVarGuard {
        _lock: MutexGuard<'static, ()>,
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            static LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
            let lock = LOCK
                .get_or_init(|| std::sync::Mutex::new(()))
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let previous = std::env::var(key).ok();
            unsafe {
                std::env::set_var(key, value);
            }
            Self {
                _lock: lock,
                key,
                previous,
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(previous) = &self.previous {
                unsafe {
                    std::env::set_var(self.key, previous);
                }
            } else {
                unsafe {
                    std::env::remove_var(self.key);
                }
            }
        }
    }

    fn write_plugin_manifest(dir: &Path, name: &str, manifest: serde_json::Value) {
        let plugin_dir = dir.join(name);
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    }

    #[test]
    fn collect_launch_spec_filters_env_and_uses_project_root_cwd() {
        let dir = std::env::temp_dir().join(format!(
            "clankers-plugin-stdio-runtime-test-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        write_plugin_manifest(
            &dir,
            "stdio-env-filter",
            serde_json::json!({
                "name": "stdio-env-filter",
                "version": "0.1.0",
                "kind": "stdio",
                "stdio": {
                    "command": "/bin/echo",
                    "working_dir": "project-root",
                    "env_allowlist": ["GITHUB_TOKEN"],
                    "sandbox": "inherit"
                }
            }),
        );

        let _github = EnvVarGuard::set("GITHUB_TOKEN", "gh-secret");

        let mut manager = PluginManager::new(dir.clone(), None);
        manager.discover();
        let manager = Arc::new(Mutex::new(manager));
        let launch = collect_launch_spec(&manager, "stdio-env-filter", &StdioBootstrapConfig {
            cwd: dir.clone(),
            mode: PluginRuntimeMode::Standalone,
        })
        .unwrap();

        assert_eq!(launch.command, "/bin/echo");
        assert_eq!(launch.cwd, dir);
        assert_eq!(launch.env, vec![("GITHUB_TOKEN".to_string(), "gh-secret".to_string())]);
        assert!(matches!(launch.sandbox, LaunchSandbox::Inherit));
    }

    #[test]
    fn collect_restricted_policy_derives_state_dir_writable_roots_and_effective_network_permission() {
        let root = std::env::temp_dir().join(format!(
            "clankers-plugin-stdio-restricted-policy-test-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let plugins_dir = root.join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        write_plugin_manifest(
            &plugins_dir,
            "stdio-restricted-policy",
            serde_json::json!({
                "name": "stdio-restricted-policy",
                "version": "0.1.0",
                "kind": "stdio",
                "permissions": ["fs:write"],
                "stdio": {
                    "command": "/bin/echo",
                    "sandbox": "restricted",
                    "writable_roots": ["build/output", "build/output"],
                    "allow_network": true
                }
            }),
        );
        write_plugin_manifest(
            &plugins_dir,
            "stdio-restricted-policy-net",
            serde_json::json!({
                "name": "stdio-restricted-policy-net",
                "version": "0.1.0",
                "kind": "stdio",
                "permissions": ["net"],
                "stdio": {
                    "command": "/bin/echo",
                    "sandbox": "restricted",
                    "writable_roots": ["cache"],
                    "allow_network": true
                }
            }),
        );

        let mut manager = PluginManager::new(plugins_dir.clone(), None);
        manager.discover();
        let bootstrap = StdioBootstrapConfig {
            cwd: root.clone(),
            mode: PluginRuntimeMode::Standalone,
        };

        let info = manager.get("stdio-restricted-policy").unwrap();
        let policy =
            collect_restricted_policy(&manager, info, info.manifest.stdio.as_ref().unwrap(), &bootstrap, "/bin/echo")
                .unwrap();
        assert_eq!(policy.state_dir, root.join("plugin-state").join("stdio-restricted-policy"));
        assert_eq!(policy.writable_roots, vec![
            root.join("plugin-state").join("stdio-restricted-policy"),
            root.join("build/output"),
        ]);
        assert!(!policy.allow_network, "logical net permission should still be required");

        let info = manager.get("stdio-restricted-policy-net").unwrap();
        let policy =
            collect_restricted_policy(&manager, info, info.manifest.stdio.as_ref().unwrap(), &bootstrap, "/bin/echo")
                .unwrap();
        assert!(policy.allow_network, "sandbox allow_network + logical net permission should allow network");
    }

    #[test]
    fn collect_restricted_policy_rejects_roots_outside_project_root() {
        let root = std::env::temp_dir().join(format!(
            "clankers-plugin-stdio-restricted-policy-invalid-test-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let plugins_dir = root.join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        write_plugin_manifest(
            &plugins_dir,
            "stdio-restricted-invalid",
            serde_json::json!({
                "name": "stdio-restricted-invalid",
                "version": "0.1.0",
                "kind": "stdio",
                "stdio": {
                    "command": "/bin/echo",
                    "sandbox": "restricted",
                    "writable_roots": ["../escape"]
                }
            }),
        );

        let mut manager = PluginManager::new(plugins_dir.clone(), None);
        manager.discover();
        let bootstrap = StdioBootstrapConfig {
            cwd: root.clone(),
            mode: PluginRuntimeMode::Standalone,
        };

        let info = manager.get("stdio-restricted-invalid").unwrap();
        let error =
            collect_restricted_policy(&manager, info, info.manifest.stdio.as_ref().unwrap(), &bootstrap, "/bin/echo")
                .unwrap_err();
        assert!(error.contains("must stay within the project root"), "error: {error}");
    }
}
