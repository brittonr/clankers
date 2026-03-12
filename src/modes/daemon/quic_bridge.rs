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

use clankers_controller::transport::DaemonState;
use clankers_protocol::DaemonEvent;
use clankers_protocol::SessionCommand;
use clankers_protocol::types::AttachResponse;
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
pub async fn handle_daemon_quic_connection(
    conn: iroh::endpoint::Connection,
    state: Arc<Mutex<DaemonState>>,
    factory: Arc<SessionFactory>,
    shutdown: tokio::sync::watch::Receiver<bool>,
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
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_daemon_stream(send, recv, state, factory, shutdown).await {
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
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), clankers_protocol::FrameError> {
    // Read the first frame to determine the stream type.
    let request: DaemonRequest = read_quic_frame(&mut recv).await?;

    match request {
        DaemonRequest::Control { command } => {
            handle_control_stream(command, &mut send, &state, &factory, &shutdown).await?;
        }
        DaemonRequest::Attach { handshake } => {
            handle_attach_stream(handshake, send, recv, &state).await?;
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
    shutdown: &tokio::sync::watch::Receiver<bool>,
) -> Result<(), clankers_protocol::FrameError> {
    debug!("QUIC control command: {command:?}");

    let response = match command {
        clankers_protocol::ControlCommand::CreateSession {
            model,
            system_prompt,
            token,
        } => {
            if token.is_none() {
                warn!("QUIC CreateSession rejected: no auth token");
                clankers_protocol::ControlResponse::Error {
                    message: "authentication token required for remote session creation".to_string(),
                }
            } else {
                create_session_over_quic(model, system_prompt, state, factory, shutdown).await
            }
        }
        other => {
            let st = state.lock().await;
            dispatch_readonly_control(other, &st)
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
) -> clankers_protocol::ControlResponse {
    use clankers_protocol::ControlCommand;
    use clankers_protocol::ControlResponse;

    match cmd {
        ControlCommand::ListSessions => ControlResponse::Sessions(state.session_summaries()),
        ControlCommand::Status => ControlResponse::Status(state.status()),
        ControlCommand::ProcessTree => ControlResponse::Tree(vec![]),
        ControlCommand::KillSession { session_id } => {
            if let Some(handle) = state.sessions.get(&session_id) {
                let _ = handle.cmd_tx.send(SessionCommand::Disconnect);
                ControlResponse::Killed
            } else {
                ControlResponse::Error {
                    message: format!("session '{session_id}' not found"),
                }
            }
        }
        ControlCommand::AttachSession { session_id } => {
            if let Some(handle) = state.sessions.get(&session_id) {
                ControlResponse::Attached {
                    socket_path: handle.socket_path.to_string_lossy().into_owned(),
                }
            } else {
                ControlResponse::Error {
                    message: format!("session '{session_id}' not found"),
                }
            }
        }
        ControlCommand::Shutdown => ControlResponse::ShuttingDown,
        ControlCommand::CreateSession { .. } => ControlResponse::Error {
            message: "internal: CreateSession routed to readonly dispatch".to_string(),
        },
    }
}

/// Create a new session, wiring up the controller and session socket.
///
/// Mirrors `socket_bridge::handle_control` CreateSession branch.
async fn create_session_over_quic(
    model: Option<String>,
    system_prompt: Option<String>,
    state: &Arc<Mutex<DaemonState>>,
    factory: &Arc<SessionFactory>,
    shutdown: &tokio::sync::watch::Receiver<bool>,
) -> clankers_protocol::ControlResponse {
    use clankers_controller::SessionController;
    use clankers_controller::config::ControllerConfig;
    use clankers_controller::transport::SessionHandle;
    use clankers_controller::transport::session_socket_path;
    use clankers_tui_types::SubagentEvent;
    use tokio::sync::mpsc;

    let session_id = clankers_message::generate_id();
    let model = model.unwrap_or_else(|| factory.default_model.clone());
    let system_prompt = system_prompt.unwrap_or_else(|| factory.default_system_prompt.clone());

    let (panel_tx, panel_rx) = mpsc::unbounded_channel::<SubagentEvent>();
    let tools = factory.build_tools_with_panel_tx(panel_tx);

    let agent = crate::agent::builder::AgentBuilder::new(
        Arc::clone(&factory.provider),
        factory.settings.clone(),
        model.clone(),
        system_prompt.clone(),
    )
    .with_tools(tools)
    .build();

    let config = ControllerConfig {
        session_id: session_id.clone(),
        model: model.clone(),
        system_prompt: Some(system_prompt),
        ..Default::default()
    };

    let controller = SessionController::new(agent, config);

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<SessionCommand>();
    let (event_tx, _) = broadcast::channel::<DaemonEvent>(256);
    let socket_path = session_socket_path(&session_id);

    {
        let mut st = state.lock().await;
        st.sessions.insert(session_id.clone(), SessionHandle {
            session_id: session_id.clone(),
            model: model.clone(),
            turn_count: 0,
            last_active: chrono::Utc::now().to_rfc3339(),
            client_count: 0,
            cmd_tx: cmd_tx.clone(),
            event_tx: event_tx.clone(),
            socket_path: socket_path.clone(),
        });
    }

    // Spawn session driver
    let driver_event_tx = event_tx.clone();
    let driver_session_id = session_id.clone();
    tokio::spawn(async move {
        super::socket_bridge::run_session_driver_pub(controller, cmd_rx, driver_event_tx, driver_session_id, panel_rx)
            .await;
    });

    // Spawn the Unix session socket too (local clients can still connect)
    let sock_shutdown = shutdown.clone();
    let sock_cmd_tx = cmd_tx.clone();
    let sock_event_tx = event_tx.clone();
    let sock_session_id = session_id.clone();
    tokio::spawn(async move {
        clankers_controller::transport::run_session_socket(
            sock_session_id,
            sock_cmd_tx,
            sock_event_tx,
            sock_shutdown,
        )
        .await;
    });

    info!("created session {session_id} via QUIC (model: {model})");
    clankers_protocol::ControlResponse::Created {
        session_id,
        socket_path: socket_path.to_string_lossy().into_owned(),
    }
}

/// Handle a session attach over QUIC.
///
/// The QUIC bidirectional stream carries the same protocol as a Unix session
/// socket: DaemonEvent frames (daemon → client) and SessionCommand frames
/// (client → daemon).
async fn handle_attach_stream(
    handshake: clankers_protocol::Handshake,
    mut send: iroh::endpoint::SendStream,
    mut recv: iroh::endpoint::RecvStream,
    state: &Arc<Mutex<DaemonState>>,
) -> Result<(), clankers_protocol::FrameError> {
    // Validate protocol version
    if handshake.protocol_version != PROTOCOL_VERSION {
        let resp = AttachResponse::Error {
            message: format!(
                "unsupported protocol version {} (expected {PROTOCOL_VERSION})",
                handshake.protocol_version,
            ),
        };
        write_quic_frame(&mut send, &resp).await?;
        send.finish().ok();
        return Ok(());
    }

    // Require token for remote QUIC connections
    if handshake.token.is_none() {
        warn!("QUIC attach rejected: no auth token in handshake");
        let resp = AttachResponse::Error {
            message: "authentication token required for remote connections".to_string(),
        };
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
                    let resp = AttachResponse::Error {
                        message: "no sessions available".to_string(),
                    };
                    write_quic_frame(&mut send, &resp).await?;
                    send.finish().ok();
                    return Ok(());
                }
            }
        }
    };

    let (cmd_tx, mut event_rx) = {
        let st = state.lock().await;
        match st.sessions.get(&session_id) {
            Some(handle) => (handle.cmd_tx.clone(), handle.event_tx.subscribe()),
            None => {
                let resp = AttachResponse::Error {
                    message: format!("session '{session_id}' not found"),
                };
                write_quic_frame(&mut send, &resp).await?;
                send.finish().ok();
                return Ok(());
            }
        }
    };

    info!("QUIC attach to session {session_id} from {}", handshake.client_name);

    // Send attach success
    let resp = AttachResponse::Ok {
        session_id: session_id.clone(),
    };
    write_quic_frame(&mut send, &resp).await?;

    // Send SessionInfo (same as Unix socket flow)
    let session_info = DaemonEvent::SessionInfo {
        session_id: session_id.clone(),
        model: String::new(),
        system_prompt_hash: String::new(),
    };
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
        if cmd_tx.send(cmd).is_err() || is_disconnect {
            break;
        }
    }

    write_task.abort();
    info!("QUIC attach ended for session {session_id}");
    Ok(())
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
    send.write_all(&len).await.map_err(|e| {
        clankers_protocol::FrameError::Io(std::io::Error::other(e.to_string()))
    })?;
    send.write_all(&data).await.map_err(|e| {
        clankers_protocol::FrameError::Io(std::io::Error::other(e.to_string()))
    })?;
    Ok(())
}

async fn read_quic_frame<T: serde::de::DeserializeOwned>(
    recv: &mut iroh::endpoint::RecvStream,
) -> Result<T, clankers_protocol::FrameError> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf).await.map_err(|e| {
        clankers_protocol::FrameError::Io(std::io::Error::other(e.to_string()))
    })?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 10_000_000 {
        return Err(clankers_protocol::FrameError::TooLarge { size: len });
    }
    let mut data = vec![0u8; len];
    recv.read_exact(&mut data).await.map_err(|e| {
        clankers_protocol::FrameError::Io(std::io::Error::other(e.to_string()))
    })?;
    let value = serde_json::from_slice(&data)?;
    Ok(value)
}
