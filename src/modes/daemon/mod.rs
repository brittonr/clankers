//! Daemon mode — headless agent that listens on iroh and Matrix.
//!
//! Runs as a long-lived background process. Incoming messages from either
//! transport are routed to per-sender agent sessions. Responses are sent
//! back through the originating channel.
//!
//! ## Transport: iroh
//!
//! Uses ALPN negotiation on the iroh QUIC endpoint:
//! - `clankers/rpc/1` — existing JSON-RPC protocol (ping, status, prompt, file)
//! - `clankers/chat/1` — conversational channel with persistent sessions
//!
//! ## Transport: Matrix
//!
//! Listens for `ClankersEvent::Text` (human messages) and `ClankersEvent::Request`
//! in joined rooms. Responses are sent back as `matrix_send`.

use std::sync::Arc;

use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::config::ClankersPaths;
use crate::error::Result;
use crate::modes::rpc::iroh;
use crate::provider::Provider;
use crate::tools::Tool;

mod config;
mod handlers;
mod session_store;
pub mod socket_bridge;

// Re-export public types
pub(crate) use config::ALPN_CHAT;
pub use config::DaemonConfig;
pub(crate) use config::ProactiveConfig;
use handlers::handle_iroh_connection;
pub(crate) use session_store::SessionKey;
pub(crate) use session_store::SessionStore;
use session_store::create_auth_layer;

// ── Daemon entry point ──────────────────────────────────────────────────────

/// Start the daemon. Blocks until cancelled.
///
/// Setup is split into focused phases: identity/auth, session store, iroh
/// endpoint, background tasks. Each phase is a helper under 70 lines.
pub async fn run_daemon(
    provider: Arc<dyn Provider>,
    tools: Vec<Arc<dyn Tool>>,
    config: DaemonConfig,
    paths: &ClankersPaths,
) -> Result<()> {
    let cancel = CancellationToken::new();

    // Phase 1: Identity and auth
    let identity_path = iroh::identity_path(paths);
    let identity = iroh::Identity::load_or_generate(&identity_path);
    let db_path = paths.global_config_dir.join("clankers.db");
    let auth_layer = create_auth_layer(&db_path, &identity);

    // Phase 2: Session store
    let store = Arc::new(RwLock::new(SessionStore::new(
        Arc::clone(&provider),
        tools.clone(),
        config.settings.clone(),
        config.model.clone(),
        config.system_prompt.clone(),
        paths.global_sessions_dir.clone(),
        config.max_sessions,
        auth_layer.clone(),
    )));

    // Phase 3: iroh endpoint + ACL
    let (endpoint, acl) = build_endpoint(&identity, &config, paths).await?;
    let acl = Arc::new(acl);
    let node_id = identity.public_key();

    print_startup_banner(&config, &node_id);

    // Phase 4: Unix domain socket control plane
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let socket_handle = spawn_socket_control_plane(
        &provider,
        &tools,
        &config,
        shutdown_rx.clone(),
    );

    // Phase 5: Background tasks
    let rpc_state = build_rpc_state(&config, &provider, &tools, paths);
    let iroh_handle =
        spawn_iroh_accept_loop(endpoint.clone(), Arc::clone(&store), Arc::clone(&acl), rpc_state, cancel.clone());

    let matrix_handle = spawn_matrix_bridge(&config, &store, paths, cancel.clone());
    spawn_heartbeat(&config, &identity, paths, cancel.clone()).await;
    spawn_status_logger(Arc::clone(&store), cancel.clone());
    spawn_idle_reaper(&config, Arc::clone(&store), cancel.clone());

    let ctrl_sock = clankers_controller::transport::control_socket_path();
    let log_path = clankers_controller::transport::daemon_log_path();
    println!("\nListening... (Ctrl+C to stop)\n");
    println!("Chat:    clankers rpc prompt {} \"hello\"", node_id);
    println!("Ping:    clankers rpc ping {}", node_id);
    println!("Control: {}", ctrl_sock.display());
    println!("Logs:    {}", log_path.display());
    println!("Status:  clankers daemon status");
    println!("Attach:  clankers attach");

    // Phase 6: Wait for shutdown
    tokio::signal::ctrl_c().await.ok();
    println!("\nShutting down...");
    cancel.cancel();
    let _ = shutdown_tx.send(true);

    iroh_handle.await.ok();
    socket_handle.await.ok();
    if let Some(h) = matrix_handle {
        h.await.ok();
    }

    clankers_controller::transport::cleanup_socket_dir();
    let store = store.read().await;
    println!("Daemon stopped ({} sessions served).", store.len());
    Ok(())
}

fn spawn_socket_control_plane(
    provider: &Arc<dyn Provider>,
    tools: &[Arc<dyn Tool>],
    config: &DaemonConfig,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    // Init socket directory (PID file, stale cleanup)
    if let Err(e) = clankers_controller::transport::init_socket_dir() {
        warn!("socket dir init failed: {e} — control socket disabled");
        return tokio::spawn(async {});
    }

    let daemon_state = Arc::new(tokio::sync::Mutex::new(
        clankers_controller::transport::DaemonState::new(),
    ));

    let factory = Arc::new(socket_bridge::SessionFactory {
        provider: Arc::clone(provider),
        tools: tools.to_vec(),
        settings: config.settings.clone(),
        default_model: config.model.clone(),
        default_system_prompt: config.system_prompt.clone(),
    });

    tokio::spawn(async move {
        socket_bridge::run_control_socket_with_factory(daemon_state, factory, shutdown_rx).await;
    })
}

// ── Setup helpers ───────────────────────────────────────────────────────────

/// Build the iroh endpoint with mDNS and both ALPNs, plus the ACL.
async fn build_endpoint(
    identity: &iroh::Identity,
    config: &DaemonConfig,
    paths: &ClankersPaths,
) -> Result<(::iroh::Endpoint, iroh::AccessControl)> {
    let mdns_service = ::iroh::address_lookup::MdnsAddressLookup::builder().service_name("_clankers._udp.local.");

    let endpoint = ::iroh::Endpoint::builder()
        .secret_key(identity.secret_key.clone())
        .alpns(vec![iroh::ALPN.to_vec(), ALPN_CHAT.to_vec()])
        .address_lookup(mdns_service)
        .bind()
        .await
        .map_err(|e| crate::error::Error::Provider {
            message: format!("Failed to bind iroh endpoint: {e}"),
        })?;

    let acl_path = iroh::allowlist_path(paths);
    let acl = if config.allow_all {
        iroh::AccessControl::open()
    } else {
        let allowed = iroh::load_allowlist(&acl_path);
        iroh::AccessControl::from_allowlist(allowed)
    };

    Ok((endpoint, acl))
}

fn print_startup_banner(config: &DaemonConfig, node_id: &::iroh::PublicKey) {
    println!("clankers daemon started");
    println!("  Node ID:  {}", node_id);
    println!(
        "  Auth:     {}",
        if config.allow_all {
            "open"
        } else {
            "allowlist + UCAN tokens"
        }
    );
    println!("  Model:    {}", config.model);
    println!("  Sessions: 0/{}", config.max_sessions);
    if !config.tags.is_empty() {
        println!("  Tags:     {}", config.tags.join(", "));
    }
    println!("  Tokens:   create with `clankers token create`");
}

/// Build the legacy RPC state for the rpc/1 ALPN.
fn build_rpc_state(
    config: &DaemonConfig,
    provider: &Arc<dyn Provider>,
    tools: &[Arc<dyn Tool>],
    paths: &ClankersPaths,
) -> Arc<iroh::ServerState> {
    let acl_path = iroh::allowlist_path(paths);
    Arc::new(iroh::ServerState {
        meta: iroh::NodeMeta {
            tags: config.tags.clone(),
            agent_names: Vec::new(),
        },
        agent: Some(iroh::RpcContext {
            provider: Arc::clone(provider),
            tools: tools.to_vec(),
            settings: config.settings.clone(),
            model: config.model.clone(),
            system_prompt: config.system_prompt.clone(),
        }),
        acl: if config.allow_all {
            iroh::AccessControl::open()
        } else {
            let allowed = iroh::load_allowlist(&acl_path);
            iroh::AccessControl::from_allowlist(allowed)
        },
        receive_dir: Some(paths.global_config_dir.join("received")),
    })
}

// ── Background task spawners ────────────────────────────────────────────────

fn spawn_iroh_accept_loop(
    endpoint: ::iroh::Endpoint,
    store: Arc<RwLock<SessionStore>>,
    acl: Arc<iroh::AccessControl>,
    rpc_state: Arc<iroh::ServerState>,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        info!("iroh accept loop started");
        loop {
            tokio::select! {
                incoming = endpoint.accept() => {
                    let Some(incoming) = incoming else { break };
                    let store = Arc::clone(&store);
                    let acl = Arc::clone(&acl);
                    let rpc_state = Arc::clone(&rpc_state);

                    tokio::spawn(async move {
                        if let Err(e) = handle_iroh_connection(incoming, store, acl, rpc_state).await {
                            warn!("iroh connection error: {e}");
                        }
                    });
                }
                () = cancel.cancelled() => break,
            }
        }
    })
}

fn spawn_matrix_bridge(
    config: &DaemonConfig,
    store: &Arc<RwLock<SessionStore>>,
    paths: &ClankersPaths,
    cancel: CancellationToken,
) -> Option<tokio::task::JoinHandle<()>> {
    if !config.enable_matrix {
        return None;
    }

    let matrix_store = Arc::clone(store);
    let matrix_paths = paths.clone();
    let matrix_allowed = config.matrix_allowed_users.clone();
    let proactive_config = ProactiveConfig {
        session_heartbeat_secs: config.session_heartbeat_secs,
        heartbeat_prompt: config.heartbeat_prompt.clone(),
        trigger_pipe_enabled: config.trigger_pipe_enabled,
    };

    Some(tokio::spawn(async move {
        if let Err(e) = super::matrix_bridge::run_matrix_bridge(
            matrix_store,
            cancel,
            &matrix_paths,
            matrix_allowed,
            proactive_config,
        )
        .await
        {
            error!("Matrix bridge error: {e}");
        }
    }))
}

async fn spawn_heartbeat(
    config: &DaemonConfig,
    identity: &iroh::Identity,
    paths: &ClankersPaths,
    cancel: CancellationToken,
) {
    if config.heartbeat_secs == 0 {
        return;
    }

    let registry_path = crate::modes::rpc::peers::registry_path(paths);
    let interval = std::time::Duration::from_secs(config.heartbeat_secs);
    let hb_endpoint = Arc::new(
        iroh::start_endpoint(identity)
            .await
            .unwrap_or_else(|_| panic!("failed to start heartbeat endpoint")),
    );
    tokio::spawn(iroh::run_heartbeat(hb_endpoint, registry_path, interval, cancel));
}

fn spawn_status_logger(store: Arc<RwLock<SessionStore>>, cancel: CancellationToken) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let store = store.read().await;
                    info!("daemon status: {} active session(s)", store.len());
                }
                () = cancel.cancelled() => break,
            }
        }
    });
}

fn spawn_idle_reaper(config: &DaemonConfig, store: Arc<RwLock<SessionStore>>, cancel: CancellationToken) {
    if config.idle_timeout_secs == 0 {
        return;
    }

    let idle_timeout = std::time::Duration::from_secs(config.idle_timeout_secs);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let reaped = store.write().await.reap_idle(idle_timeout);
                    if reaped > 0 {
                        info!("Reaped {} idle session(s)", reaped);
                    }
                }
                () = cancel.cancelled() => break,
            }
        }
    });
}
