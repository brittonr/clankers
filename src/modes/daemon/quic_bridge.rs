//! Bridge between iroh QUIC streams and daemon sessions.
//!
//! Handles `clankers/daemon/1` ALPN connections. Each bidirectional stream
//! starts with a [`DaemonRequest`] frame that selects control vs session mode:
//!
//! - **Control**: one-shot command/response (list sessions, create, kill)
//! - **Attach**: long-lived bidirectional SessionCommand/DaemonEvent flow
//!
//! The session attach path reuses the same framing as Unix domain sockets,
//! so `ClientAdapter` works unmodified on the client side.

use std::sync::Arc;

use clankers_agent::CapabilityGate;
use clankers_controller::transport::DaemonState;
use clankers_controller::transport::SessionSocketInfo;
use clankers_controller::transport_convert::attach_error;
use clankers_controller::transport_convert::attach_ok;
use clankers_controller::transport_convert::control_attached;
use clankers_controller::transport_convert::control_created;
use clankers_controller::transport_convert::control_error;
use clankers_controller::transport_convert::control_killed;
use clankers_controller::transport_convert::control_plugins;
use clankers_controller::transport_convert::control_restarting;
use clankers_controller::transport_convert::control_sessions;
use clankers_controller::transport_convert::control_shutting_down;
use clankers_controller::transport_convert::control_status;
use clankers_controller::transport_convert::control_tree;
use clankers_controller::transport_convert::session_info_event;
use clankers_protocol::SessionCommand;
use clankers_protocol::types::DaemonRequest;
use clankers_protocol::types::PROTOCOL_VERSION;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use tracing::debug;
use tracing::info;
use tracing::warn;

use super::socket_bridge::SessionFactory;

/// ALPN for daemon control plane over QUIC.
pub const ALPN_DAEMON: &[u8] = clankers_protocol::types::ALPN_DAEMON;

/// Handle a single QUIC connection on the daemon ALPN.
///
/// Each bidirectional stream is dispatched independently: control streams
/// get a single response, attach streams stay open for the session lifetime.
///
/// When `should_skip_token_check` is true (daemon started with `--allow-all`),
/// the per-stream token requirement is bypassed — the ACL already
/// admitted this peer at the connection level.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(unbounded_loop, reason = "event loop; bounded by connection close")
)]
pub async fn handle_daemon_quic_connection(
    conn: iroh::endpoint::Connection,
    state: Arc<Mutex<DaemonState>>,
    factory: Arc<SessionFactory>,
    registry: clanker_actor::ProcessRegistry,
    shutdown: tokio::sync::watch::Receiver<bool>,
    should_skip_token_check: bool,
    auth: Option<Arc<super::session_store::AuthLayer>>,
) {
    let remote = conn.remote_id();
    info!("daemon QUIC connection from {}", remote.fmt_short());

    loop {
        let (send, recv) = match conn.accept_bi().await {
            Ok(streams) => streams,
            Err(_) => break,
        };

        let state = Arc::clone(&state);
        let factory = Arc::clone(&factory);
        let registry = registry.clone();
        let shutdown = shutdown.clone();
        let auth = auth.clone();
        tokio::spawn(async move {
            if let Err(e) =
                handle_daemon_stream(send, recv, state, factory, registry, shutdown, should_skip_token_check, auth)
                    .await
            {
                debug!("daemon QUIC stream ended: {e}");
            }
        });
    }

    info!("daemon QUIC connection closed from {}", remote.fmt_short());
}

/// Handle a single bidirectional stream on the daemon ALPN.
async fn handle_daemon_stream(
    mut send: iroh::endpoint::SendStream,
    mut recv: iroh::endpoint::RecvStream,
    state: Arc<Mutex<DaemonState>>,
    factory: Arc<SessionFactory>,
    registry: clanker_actor::ProcessRegistry,
    shutdown: tokio::sync::watch::Receiver<bool>,
    should_skip_token_check: bool,
    auth: Option<Arc<super::session_store::AuthLayer>>,
) -> Result<(), clankers_protocol::FrameError> {
    // Read the first frame to determine the stream type.
    let request: DaemonRequest = read_quic_frame(&mut recv).await?;

    match request {
        DaemonRequest::Control { command } => {
            handle_control_stream(
                command,
                &mut send,
                &state,
                &factory,
                &registry,
                &shutdown,
                should_skip_token_check,
                auth.as_deref(),
            )
            .await?;
        }
        DaemonRequest::Attach { handshake } => {
            handle_attach_stream(
                handshake,
                send,
                recv,
                &state,
                &factory,
                &registry,
                &shutdown,
                should_skip_token_check,
                auth.as_deref(),
            )
            .await?;
        }
    }

    Ok(())
}

/// Handle a control command: process and send one response.
async fn handle_control_stream(
    command: clankers_protocol::ControlCommand,
    send: &mut iroh::endpoint::SendStream,
    state: &Arc<Mutex<DaemonState>>,
    factory: &Arc<SessionFactory>,
    registry: &clanker_actor::ProcessRegistry,
    shutdown: &tokio::sync::watch::Receiver<bool>,
    should_skip_token_check: bool,
    auth: Option<&super::session_store::AuthLayer>,
) -> Result<(), clankers_protocol::FrameError> {
    debug!("QUIC control command: {command:?}");

    let response = match command {
        clankers_protocol::ControlCommand::CreateSession {
            model,
            system_prompt,
            token,
            cwd,
            thinking_level,
            ..
        } => {
            if !should_skip_token_check && token.is_none() {
                warn!("QUIC CreateSession rejected: no auth token");
                control_error("authentication token required for remote session creation")
            } else {
                // Verify public UCAN token and build the call-time tool gate when auth is available.
                let public_auth = if let Some(token_b64) = token.as_deref()
                    && let Some(auth) = auth
                {
                    let request = super::session_store::session_create_admission_request();
                    match auth.verify_credential_base64(token_b64, &request) {
                        Ok((credential, _receipt)) => Some(match cwd.as_deref() {
                            Some(file_root) => auth.public_tool_authorization_for_file_root(credential, file_root),
                            None => auth.public_tool_authorization(credential),
                        }),
                        Err(error) => {
                            warn!("QUIC CreateSession: public UCAN/Basalt verification failed: {error}");
                            return write_quic_frame(
                                send,
                                &control_error(format!("token verification failed: {error}")),
                            )
                            .await;
                        }
                    }
                } else {
                    None // should_skip_token_check or no auth layer = full access
                };

                if let (Some(public_auth), Some(requested_model)) = (public_auth.as_ref(), model.as_deref()) {
                    let gate = crate::capability_gate::PublicUcanCapabilityGate::new(public_auth.clone());
                    if let Err(error) = gate.check_model_switch(requested_model) {
                        warn!("QUIC CreateSession: requested model denied by public UCAN/Basalt: {error}");
                        return write_quic_frame(send, &control_error(format!("model selection denied: {error}")))
                            .await;
                    }
                }

                create_session_over_quic(
                    model,
                    system_prompt,
                    thinking_level,
                    state,
                    factory,
                    registry,
                    shutdown,
                    None,
                    public_auth,
                )
                .await
            }
        }
        other => {
            let st = state.lock().await;
            dispatch_readonly_control(other, &st, factory)
        }
    };

    write_quic_frame(send, &response).await?;
    send.finish().ok();
    Ok(())
}

/// Dispatch a read-only control command (no session creation).
fn dispatch_readonly_control(
    cmd: clankers_protocol::ControlCommand,
    state: &DaemonState,
    factory: &Arc<SessionFactory>,
) -> clankers_protocol::ControlResponse {
    use clankers_protocol::ControlCommand;

    match cmd {
        ControlCommand::ListSessions => control_sessions(state),
        ControlCommand::Status => control_status(state),
        ControlCommand::ProcessTree => control_tree(vec![]),
        ControlCommand::KillSession { session_id } => {
            if let Some(handle) = state.sessions.get(&session_id) {
                if let Some(ref tx) = handle.cmd_tx {
                    tx.send(SessionCommand::Disconnect).ok();
                }
                control_killed()
            } else {
                control_error(format!("session '{session_id}' not found"))
            }
        }
        ControlCommand::AttachSession { session_id } => {
            if let Some(handle) = state.sessions.get(&session_id) {
                control_attached(&handle.socket_path)
            } else {
                control_error(format!("session '{session_id}' not found"))
            }
        }
        ControlCommand::Shutdown => control_shutting_down(),
        ControlCommand::RestartDaemon => control_restarting(),
        ControlCommand::CreateSession { .. } => control_error("internal: CreateSession routed to readonly dispatch"),
        ControlCommand::ListPlugins => {
            let summaries = if let Some(ref pm) = factory.plugin_manager {
                crate::plugin::build_protocol_plugin_summaries(pm)
            } else {
                Vec::new()
            };
            control_plugins(summaries)
        }
    }
}

/// Create a new session, wiring up the controller and session socket.
///
/// Uses `spawn_agent_process` so the session lives in the actor registry
/// with proper link/monitor semantics and in-process subagent support.
///
/// `capabilities` — legacy UCAN capabilities from the verified token (None = full access).
/// `public_auth` installs public UCAN + Basalt call-time tool enforcement.
async fn create_session_over_quic(
    model: Option<String>,
    system_prompt: Option<String>,
    thinking_level: Option<String>,
    state: &Arc<Mutex<DaemonState>>,
    factory: &Arc<SessionFactory>,
    registry: &clanker_actor::ProcessRegistry,
    shutdown: &tokio::sync::watch::Receiver<bool>,
    capabilities: Option<Vec<clankers_ucan::Capability>>,
    public_auth: Option<crate::capability_gate::PublicUcanToolAuthorization>,
) -> clankers_protocol::ControlResponse {
    use clankers_controller::transport::SessionHandle;
    use clankers_controller::transport::session_socket_path;

    let session_id = clanker_message::transcript::generate_id();
    let resolved_model = model.clone().unwrap_or_else(|| factory.default_model.clone());

    // Spawn as an actor process in the registry (with UCAN capability enforcement)
    let spawned = super::agent_process::spawn_agent_process(
        registry,
        factory,
        session_id.clone(),
        model,
        system_prompt,
        None,
        capabilities,
        public_auth,
    );
    let cmd_tx = spawned.cmd_tx;
    let event_tx = spawned.event_tx;

    if let Some(level) = thinking_level.filter(|level| !level.trim().is_empty()) {
        cmd_tx.send(SessionCommand::SetThinkingLevel { level }).ok();
    }

    let socket_path = session_socket_path(&session_id);

    // Register in daemon state
    {
        let mut st = state.lock().await;
        st.sessions.insert(session_id.clone(), SessionHandle {
            session_id: session_id.clone(),
            model: resolved_model.clone(),
            turn_count: 0,
            last_active: chrono::Utc::now().to_rfc3339(),
            client_count: 0,
            cmd_tx: Some(cmd_tx.clone()),
            event_tx: Some(event_tx.clone()),
            socket_path: socket_path.clone(),
            state: "active".to_string(),
        });
    }

    // Write catalog entry for recovery
    if let Some(ref catalog) = factory.catalog {
        let now = chrono::Utc::now().to_rfc3339();
        catalog.insert_session(&super::session_store::SessionCatalogEntry {
            session_id: session_id.clone(),
            automerge_path: spawned.automerge_path.clone().unwrap_or_default(),
            model: resolved_model.clone(),
            created_at: now.clone(),
            last_active: now,
            turn_count: 0,
            state: super::session_store::SessionLifecycle::Active,
        });
    }

    // Bind the Unix session socket before replying so local attaches cannot race it.
    let listener = match clankers_controller::transport::bind_session_socket(&session_id) {
        Ok(listener) => listener,
        Err(e) => {
            {
                let mut st = state.lock().await;
                st.remove_session(&session_id);
            }
            if let Some(ref catalog) = factory.catalog {
                catalog.set_state(&session_id, super::session_store::SessionLifecycle::Tombstoned);
            }
            return control_error(format!("failed to bind session socket for {session_id}: {e}"));
        }
    };
    let sock_shutdown = shutdown.clone();
    let sock_cmd_tx = cmd_tx.clone();
    let sock_event_tx = event_tx.clone();
    let sock_session_id = session_id.clone();
    tokio::spawn(async move {
        clankers_controller::transport::run_session_socket_with_listener(
            listener,
            sock_session_id,
            sock_cmd_tx,
            sock_event_tx,
            sock_shutdown,
        )
        .await;
    });

    info!("created session {session_id} via QUIC (model: {resolved_model})");
    control_created(&session_id, &socket_path)
}

/// Handle a session attach over QUIC.
///
/// The QUIC bidirectional stream carries the same protocol as a Unix session
/// socket: DaemonEvent frames (daemon → client) and SessionCommand frames
/// (client → daemon).
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(unbounded_loop, reason = "event loop; bounded by connection close")
)]
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(function_length, reason = "sequential setup/dispatch logic")
)]
async fn handle_attach_stream(
    handshake: clankers_protocol::Handshake,
    mut send: iroh::endpoint::SendStream,
    mut recv: iroh::endpoint::RecvStream,
    state: &Arc<Mutex<DaemonState>>,
    factory: &Arc<SessionFactory>,
    registry: &clanker_actor::ProcessRegistry,
    shutdown: &tokio::sync::watch::Receiver<bool>,
    should_skip_token_check: bool,
    auth: Option<&super::session_store::AuthLayer>,
) -> Result<(), clankers_protocol::FrameError> {
    // Validate protocol version
    if handshake.protocol_version != PROTOCOL_VERSION {
        let resp = attach_error(format!(
            "unsupported protocol version {} (expected {PROTOCOL_VERSION})",
            handshake.protocol_version,
        ));
        write_quic_frame(&mut send, &resp).await?;
        send.finish().ok();
        return Ok(());
    }

    // Require token for remote QUIC connections (unless --allow-all)
    if !should_skip_token_check && handshake.token.is_none() {
        warn!("QUIC attach rejected: no auth token in handshake");
        let resp = attach_error("authentication token required for remote connections");
        write_quic_frame(&mut send, &resp).await?;
        send.finish().ok();
        return Ok(());
    }

    // Find the session
    let session_id = match &handshake.session_id {
        Some(id) => id.clone(),
        None => {
            // No session specified — pick the first (or only) session
            let st = state.lock().await;
            match st.sessions.keys().next() {
                Some(id) => id.clone(),
                None => {
                    let resp = attach_error("no sessions available");
                    write_quic_frame(&mut send, &resp).await?;
                    send.finish().ok();
                    return Ok(());
                }
            }
        }
    };

    let attach_public_auth = if !should_skip_token_check
        && let Some(auth) = auth
        && let Some(token_b64) = handshake.token.as_deref()
    {
        let request = super::session_store::session_attach_admission_request(&session_id);
        match auth.verify_credential_base64(token_b64, &request) {
            Ok((credential, _receipt)) => Some(auth.public_tool_authorization(credential)),
            Err(error) => {
                warn!("QUIC attach rejected: public UCAN/Basalt denied: {error}");
                let resp = attach_error(format!("token verification failed: {error}"));
                write_quic_frame(&mut send, &resp).await?;
                send.finish().ok();
                return Ok(());
            }
        }
    } else {
        None
    };

    let (cmd_tx, event_tx, mut event_rx) = {
        let mut st = state.lock().await;

        // Check if session needs lazy recovery
        let needs_recovery = st.sessions.get(&session_id).is_some_and(|h| h.cmd_tx.is_none());

        if needs_recovery {
            match super::agent_process::recover_session(&session_id, registry, factory, &mut st, shutdown) {
                Ok((cmd_tx, event_tx)) => (cmd_tx, event_tx.clone(), event_tx.subscribe()),
                Err(e) => {
                    let resp = attach_error(format!("session recovery failed: {e}"));
                    write_quic_frame(&mut send, &resp).await?;
                    send.finish().ok();
                    return Ok(());
                }
            }
        } else {
            match st.sessions.get(&session_id) {
                Some(handle) => {
                    let Some(ref cmd_tx) = handle.cmd_tx else {
                        let resp = attach_error(format!("session '{session_id}' has no command channel"));
                        write_quic_frame(&mut send, &resp).await?;
                        send.finish().ok();
                        return Ok(());
                    };
                    let Some(ref event_tx) = handle.event_tx else {
                        let resp = attach_error(format!("session '{session_id}' has no event channel"));
                        write_quic_frame(&mut send, &resp).await?;
                        send.finish().ok();
                        return Ok(());
                    };
                    (cmd_tx.clone(), event_tx.clone(), event_tx.subscribe())
                }
                None => {
                    let resp = attach_error(format!("session '{session_id}' not found"));
                    write_quic_frame(&mut send, &resp).await?;
                    send.finish().ok();
                    return Ok(());
                }
            }
        }
    };

    info!("QUIC attach to session {session_id} from {}", handshake.client_name);

    // Send attach success
    let resp = attach_ok(&session_id);
    write_quic_frame(&mut send, &resp).await?;

    // Send SessionInfo (same as Unix socket flow)
    let session_info = session_info_event(&session_id, &SessionSocketInfo::default());
    write_quic_frame(&mut send, &session_info).await?;

    // Bidirectional relay: QUIC ↔ session channels
    //
    // Writer: broadcast events → QUIC send stream
    let write_task = tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    if write_quic_frame(&mut send, &event).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("QUIC client lagged, missed {n} events");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Reader: QUIC recv stream → session commands
    while let Ok(cmd) = read_quic_frame::<SessionCommand>(&mut recv).await {
        let is_disconnect = matches!(cmd, SessionCommand::Disconnect);
        if let Err(reason) = authorize_attached_session_command(attach_public_auth.as_ref(), &session_id, &cmd) {
            let is_prompt_like = matches!(cmd, SessionCommand::Prompt { .. } | SessionCommand::RewriteAndPrompt { .. });
            event_tx
                .send(clankers_protocol::DaemonEvent::SystemMessage {
                    text: format!("🔒 {reason}"),
                    is_error: true,
                })
                .ok();
            if is_prompt_like {
                event_tx.send(clankers_protocol::DaemonEvent::PromptDone { error: Some(reason) }).ok();
            }
            if is_disconnect {
                break;
            }
            continue;
        }
        if cmd_tx.send(cmd).is_err() || is_disconnect {
            break;
        }
    }

    write_task.abort();
    info!("QUIC attach ended for session {session_id}");
    Ok(())
}

fn authorize_attached_session_command(
    public_auth: Option<&crate::capability_gate::PublicUcanToolAuthorization>,
    session_id: &str,
    cmd: &SessionCommand,
) -> Result<(), String> {
    let Some(public_auth) = public_auth else {
        return Ok(());
    };
    let gate = crate::capability_gate::PublicUcanCapabilityGate::new(public_auth.clone());
    match cmd {
        SessionCommand::Prompt { text, .. } => gate.check_prompt(session_id, text),
        SessionCommand::RewriteAndPrompt { text } => {
            gate.check_session_manage(session_id, "rewrite_prompt")?;
            gate.check_prompt(session_id, text)
        }
        SessionCommand::SetModel { model } => gate.check_model_switch(model),
        SessionCommand::ClearHistory => gate.check_session_manage(session_id, "clear_history"),
        SessionCommand::TruncateMessages { .. } => gate.check_session_manage(session_id, "truncate_messages"),
        SessionCommand::SetThinkingLevel { .. } => gate.check_session_manage(session_id, "set_thinking_level"),
        SessionCommand::CycleThinkingLevel => gate.check_session_manage(session_id, "cycle_thinking_level"),
        SessionCommand::SeedMessages { .. } => gate.check_session_manage(session_id, "seed_messages"),
        SessionCommand::SetSystemPrompt { .. } => gate.check_session_manage(session_id, "set_system_prompt"),
        SessionCommand::SetDisabledTools { .. } => gate.check_session_manage(session_id, "set_disabled_tools"),
        SessionCommand::CompactHistory => gate.check_session_manage(session_id, "compact_history"),
        SessionCommand::StartLoop { .. } => gate.check_session_manage(session_id, "start_loop"),
        SessionCommand::StopLoop => gate.check_session_manage(session_id, "stop_loop"),
        SessionCommand::SetAutoTest { .. } => gate.check_session_manage(session_id, "set_auto_test"),
        SessionCommand::SetCapabilities { .. } => gate.check_session_manage(session_id, "set_capabilities"),
        SessionCommand::SlashCommand { command, args } => {
            authorize_attached_slash_command(&gate, session_id, command, args)
        }
        SessionCommand::Abort
        | SessionCommand::ResetCancel
        | SessionCommand::ConfirmBash { .. }
        | SessionCommand::TodoResponse { .. }
        | SessionCommand::GetSystemPrompt
        | SessionCommand::SwitchAccount { .. }
        | SessionCommand::GetToolList
        | SessionCommand::ReplayHistory
        | SessionCommand::GetCapabilities
        | SessionCommand::Disconnect
        | SessionCommand::GetPlugins => Ok(()),
    }
}

fn authorize_attached_slash_command(
    gate: &crate::capability_gate::PublicUcanCapabilityGate,
    session_id: &str,
    command: &str,
    args: &str,
) -> Result<(), String> {
    match command {
        "model" if !args.is_empty() => gate.check_model_switch(args),
        "clear" => gate.check_session_manage(session_id, "clear_history"),
        "compact" => gate.check_session_manage(session_id, "compact_history"),
        "thinking" if args.is_empty() => gate.check_session_manage(session_id, "cycle_thinking_level"),
        "thinking" => gate.check_session_manage(session_id, "set_thinking_level"),
        "stop" => gate.check_session_manage(session_id, "stop_loop"),
        "autotest" => gate.check_session_manage(session_id, "set_auto_test"),
        _ => Ok(()),
    }
}

// ── QUIC frame helpers ──────────────────────────────────────────────────────
//
// Same length-prefixed JSON framing as clankers_protocol::frame, but over
// iroh QUIC send/recv streams instead of tokio AsyncRead/AsyncWrite.

async fn write_quic_frame<T: serde::Serialize>(
    send: &mut iroh::endpoint::SendStream,
    value: &T,
) -> Result<(), clankers_protocol::FrameError> {
    let data = serde_json::to_vec(value)?;
    if data.len() > 10_000_000 {
        return Err(clankers_protocol::FrameError::TooLarge { size: data.len() });
    }
    let len = (data.len() as u32).to_be_bytes();
    send.write_all(&len)
        .await
        .map_err(|e| clankers_protocol::FrameError::Io(std::io::Error::other(e.to_string())))?;
    send.write_all(&data)
        .await
        .map_err(|e| clankers_protocol::FrameError::Io(std::io::Error::other(e.to_string())))?;
    Ok(())
}

async fn read_quic_frame<T: serde::de::DeserializeOwned>(
    recv: &mut iroh::endpoint::RecvStream,
) -> Result<T, clankers_protocol::FrameError> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf)
        .await
        .map_err(|e| clankers_protocol::FrameError::Io(std::io::Error::other(e.to_string())))?;
    let len = usize::try_from(u32::from_be_bytes(len_buf)).unwrap_or(0);
    if len > 10_000_000 {
        return Err(clankers_protocol::FrameError::TooLarge { size: len });
    }
    let mut data = vec![0u8; len];
    recv.read_exact(&mut data)
        .await
        .map_err(|e| clankers_protocol::FrameError::Io(std::io::Error::other(e.to_string())))?;
    let value = serde_json::from_slice(&data)?;
    Ok(value)
}
