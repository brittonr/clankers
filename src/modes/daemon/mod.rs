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

use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::config::ClankersPaths;
use crate::error::Result;
use crate::modes::rpc::iroh;
use crate::provider::Provider;
use crate::tools::Tool;

pub mod agent_process;
mod config;
mod handlers;
pub mod quic_bridge;
pub mod session_store;
pub mod socket_bridge;

// Re-export public types
pub(crate) use config::ALPN_CHAT;
pub use config::DaemonConfig;
pub(crate) use config::ProactiveConfig;
use session_store::create_auth_layer;

// ── Daemon entry point ──────────────────────────────────────────────────────

/// Start the daemon. Blocks until cancelled.
///
/// Setup is split into focused phases: identity/auth, session store, iroh
/// endpoint, background tasks. Each phase is a helper under 70 lines.
/// Flag indicating restart was requested (exit code 75).
static RESTART_REQUESTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

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
    let daemon_db = session_store::open_daemon_db(&db_path);
    let auth_layer = daemon_db.as_ref().and_then(|db| create_auth_layer(db, &identity));
    let session_catalog = daemon_db.as_ref().map(session_store::create_session_catalog);

    // Crash recovery: any `active` entries from a previous daemon that died
    // without a clean shutdown should be treated as `suspended`.
    if let Some(ref catalog) = session_catalog {
        let recovered = catalog.transition_all(
            session_store::SessionLifecycle::Active,
            session_store::SessionLifecycle::Suspended,
        );
        if recovered > 0 {
            info!("Recovered {recovered} stale active session(s) → suspended");
        }
    }

    // Phase 2: iroh endpoint + ACL (non-fatal — daemon works without iroh)
    let iroh_result = build_endpoint(&identity, &config, paths).await;
    let (endpoint, acl) = match iroh_result {
        Ok((ep, a)) => (Some(ep), Some(Arc::new(a))),
        Err(e) => {
            warn!("iroh endpoint unavailable: {e} — running with control socket only");
            (None, None)
        }
    };
    let node_id = identity.public_key();

    print_startup_banner(&config, &node_id);

    // Phase 3: Unix domain socket control plane + shared factory + actor registry
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let daemon_state = Arc::new(tokio::sync::Mutex::new(
        clankers_controller::transport::DaemonState::new(),
    ));

    let process_registry = clanker_actor::ProcessRegistry::new();

    let session_factory = Arc::new(socket_bridge::SessionFactory {
        provider: Arc::clone(&provider),
        tools: tools.clone(),
        settings: config.settings.clone(),
        default_model: config.model.clone(),
        default_system_prompt: config.system_prompt.clone(),
        registry: Some(process_registry.clone()),
        catalog: session_catalog.clone(),
    });

    // Phase 3b: Populate DaemonState with suspended sessions from catalog
    if let Some(ref catalog) = session_catalog {
        let suspended = catalog.list_by_state(session_store::SessionLifecycle::Suspended);
        let key_mappings = catalog.list_keys();
        if !suspended.is_empty() {
            let mut st = daemon_state.blocking_lock();
            for entry in &suspended {
                let socket_path = clankers_controller::transport::session_socket_path(&entry.session_id);
                st.sessions.insert(entry.session_id.clone(), clankers_controller::transport::SessionHandle {
                    session_id: entry.session_id.clone(),
                    model: entry.model.clone(),
                    turn_count: entry.turn_count,
                    last_active: entry.last_active.clone(),
                    client_count: 0,
                    cmd_tx: None,
                    event_tx: None,
                    socket_path,
                    state: "suspended".to_string(),
                });
            }
            // Restore key index
            for (key, session_id) in &key_mappings {
                if st.sessions.contains_key(session_id) {
                    st.register_key(key.clone(), session_id.clone());
                }
            }
            info!("Loaded {} suspended session(s) from catalog ({} key mappings)", suspended.len(), key_mappings.len());
        }
    }

    let socket_handle = spawn_socket_control_plane_shared(
        Arc::clone(&daemon_state),
        Arc::clone(&session_factory),
        process_registry.clone(),
        shutdown_rx.clone(),
    );

    // Phase 4: Background tasks
    let iroh_handle = if let (Some(endpoint), Some(acl)) = (endpoint, acl) {
        let rpc_state = build_rpc_state(&config, &provider, &tools, paths);
        Some(spawn_iroh_accept_loop(
            endpoint.clone(),
            acl,
            rpc_state,
            Arc::clone(&daemon_state),
            Arc::clone(&session_factory),
            process_registry.clone(),
            auth_layer.clone(),
            shutdown_rx.clone(),
            cancel.clone(),
        ))
    } else {
        None
    };

    let matrix_handle = spawn_matrix_bridge(
        &config,
        &daemon_state,
        &session_factory,
        &process_registry,
        &auth_layer,
        paths,
        cancel.clone(),
    );
    spawn_heartbeat(&config, &identity, paths, cancel.clone()).await;
    spawn_status_logger(Arc::clone(&daemon_state), cancel.clone());
    spawn_idle_reaper(&config, Arc::clone(&daemon_state), process_registry.clone(), session_catalog.clone(), cancel.clone());
    spawn_catalog_updater(session_catalog.clone(), Arc::clone(&daemon_state), cancel.clone());
    spawn_catalog_gc(session_catalog.clone(), cancel.clone());

    let ctrl_sock = clankers_controller::transport::control_socket_path();
    let log_path = clankers_controller::transport::daemon_log_path();
    println!("\nListening... (Ctrl+C to stop)\n");
    println!("Chat:    clankers rpc prompt {} \"hello\"", node_id);
    println!("Ping:    clankers rpc ping {}", node_id);
    println!("Control: {}", ctrl_sock.display());
    println!("Logs:    {}", log_path.display());
    println!("Status:  clankers daemon status");
    println!("Attach:  clankers attach");

    // Phase 6: Wait for shutdown (SIGINT or SIGTERM)
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("failed to install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = sigterm.recv() => {},
    }
    println!("\nShutting down...");

    // Send Shutdown to all actor processes, then wait briefly for graceful exit.
    // Controllers flush unsaved messages to automerge during shutdown.
    process_registry.shutdown_all(std::time::Duration::from_secs(5)).await;

    // Transition all active catalog entries to suspended
    if let Some(ref catalog) = session_catalog {
        let suspended = catalog.transition_all(
            session_store::SessionLifecycle::Active,
            session_store::SessionLifecycle::Suspended,
        );
        if suspended > 0 {
            info!("Suspended {suspended} session(s) in catalog for recovery");
        }
    }

    cancel.cancel();
    let _ = shutdown_tx.send(true);

    if let Some(h) = iroh_handle {
        h.await.ok();
    }
    socket_handle.await.ok();
    if let Some(h) = matrix_handle {
        h.await.ok();
    }

    clankers_controller::transport::cleanup_socket_dir();
    let session_count = daemon_state.lock().await.sessions.len();

    if RESTART_REQUESTED.load(std::sync::atomic::Ordering::SeqCst) {
        println!("Daemon restarting ({session_count} sessions checkpointed).");
        std::process::exit(crate::commands::daemon::RESTART_EXIT_CODE);
    }

    println!("Daemon stopped ({session_count} sessions served).");
    Ok(())
}

fn spawn_socket_control_plane_shared(
    daemon_state: Arc<tokio::sync::Mutex<clankers_controller::transport::DaemonState>>,
    factory: Arc<socket_bridge::SessionFactory>,
    registry: clanker_actor::ProcessRegistry,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    // Init socket directory (PID file, stale cleanup)
    if let Err(e) = clankers_controller::transport::init_socket_dir() {
        warn!("socket dir init failed: {e} — control socket disabled");
        return tokio::spawn(async {});
    }

    tokio::spawn(async move {
        socket_bridge::run_control_socket_with_factory(daemon_state, factory, registry, shutdown_rx).await;
    })
}

// ── Setup helpers ───────────────────────────────────────────────────────────

/// Build the iroh endpoint with mDNS and both ALPNs, plus the ACL.
///
/// Set `CLANKERS_NO_MDNS=1` to skip mDNS address lookup (useful in VMs
/// or environments without multicast support).
async fn build_endpoint(
    identity: &iroh::Identity,
    config: &DaemonConfig,
    paths: &ClankersPaths,
) -> Result<(::iroh::Endpoint, iroh::AccessControl)> {
    let no_mdns = std::env::var("CLANKERS_NO_MDNS").unwrap_or_default() == "1";

    // Start from the default builder which includes DNS pkarr discovery.
    // Only add mDNS on top if not disabled.
    let mut builder = ::iroh::Endpoint::builder()
        .secret_key(identity.secret_key.clone())
        .alpns(vec![
            iroh::ALPN.to_vec(),
            ALPN_CHAT.to_vec(),
            quic_bridge::ALPN_DAEMON.to_vec(),
        ]);

    if no_mdns {
        info!("mDNS disabled (CLANKERS_NO_MDNS=1), DNS/pkarr discovery still active");
    } else {
        let mdns_service = ::iroh::address_lookup::MdnsAddressLookup::builder().service_name("_clankers._udp.local.");
        builder = builder.address_lookup(mdns_service);
    }

    let endpoint = builder
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
    acl: Arc<iroh::AccessControl>,
    rpc_state: Arc<iroh::ServerState>,
    daemon_state: Arc<tokio::sync::Mutex<clankers_controller::transport::DaemonState>>,
    session_factory: Arc<socket_bridge::SessionFactory>,
    registry: clanker_actor::ProcessRegistry,
    auth: Option<Arc<session_store::AuthLayer>>,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        info!("iroh accept loop started");
        loop {
            tokio::select! {
                incoming = endpoint.accept() => {
                    let Some(incoming) = incoming else { break };
                    let acl = Arc::clone(&acl);
                    let rpc_state = Arc::clone(&rpc_state);
                    let daemon_state = Arc::clone(&daemon_state);
                    let session_factory = Arc::clone(&session_factory);
                    let registry = registry.clone();
                    let auth = auth.clone();
                    let shutdown_rx = shutdown_rx.clone();

                    tokio::spawn(async move {
                        let conn = match incoming.await {
                            Ok(c) => c,
                            Err(e) => {
                                warn!("iroh connection failed: {e}");
                                return;
                            }
                        };

                        let remote = conn.remote_id();
                        if !acl.is_allowed(&remote) {
                            warn!("Rejected unauthorized peer {}", remote.fmt_short());
                            conn.close(1u32.into(), b"unauthorized");
                            return;
                        }

                        let skip_token = acl.allow_all;
                        let alpn = conn.alpn().to_vec();
                        match alpn.as_slice() {
                            x if x == quic_bridge::ALPN_DAEMON => {
                                quic_bridge::handle_daemon_quic_connection(
                                    conn,
                                    daemon_state,
                                    session_factory,
                                    registry,
                                    shutdown_rx,
                                    skip_token,
                                    auth,
                                ).await;
                            }
                            _ => {
                                if let Err(e) = handle_iroh_connection_from_conn(
                                    conn,
                                    rpc_state,
                                    daemon_state,
                                    session_factory,
                                    registry,
                                    auth,
                                ).await {
                                    warn!("iroh connection error: {e}");
                                }
                            }
                        }
                    });
                }
                () = cancel.cancelled() => break,
            }
        }
    })
}

/// Handle an already-accepted iroh connection (rpc/1 or chat/1).
///
/// This wraps the existing handlers but skips the ACL check (already done
/// in the accept loop) and incoming.await (already awaited).
async fn handle_iroh_connection_from_conn(
    conn: ::iroh::endpoint::Connection,
    rpc_state: Arc<iroh::ServerState>,
    daemon_state: Arc<tokio::sync::Mutex<clankers_controller::transport::DaemonState>>,
    session_factory: Arc<socket_bridge::SessionFactory>,
    registry: clanker_actor::ProcessRegistry,
    auth: Option<Arc<session_store::AuthLayer>>,
) -> Result<()> {
    let remote = conn.remote_id();
    let alpn = conn.alpn();
    info!("Connection from {} (ALPN: {:?})", remote.fmt_short(), String::from_utf8_lossy(alpn));

    match &*alpn {
        x if x == ALPN_CHAT => {
            handlers::handle_chat_connection(
                conn,
                daemon_state,
                registry,
                session_factory,
                auth,
                &remote.to_string(),
            ).await;
        }
        x if x == iroh::ALPN => {
            handlers::handle_rpc_v1_connection(conn, rpc_state, auth).await;
        }
        _ => {
            warn!("Unknown ALPN: {:?}", String::from_utf8_lossy(alpn));
            conn.close(2u32.into(), b"unknown alpn");
        }
    }

    Ok(())
}

fn spawn_matrix_bridge(
    config: &DaemonConfig,
    daemon_state: &Arc<tokio::sync::Mutex<clankers_controller::transport::DaemonState>>,
    session_factory: &Arc<socket_bridge::SessionFactory>,
    registry: &clanker_actor::ProcessRegistry,
    auth: &Option<Arc<session_store::AuthLayer>>,
    paths: &ClankersPaths,
    cancel: CancellationToken,
) -> Option<tokio::task::JoinHandle<()>> {
    if !config.enable_matrix {
        return None;
    }

    let state = Arc::clone(daemon_state);
    let registry = registry.clone();
    let factory = Arc::clone(session_factory);
    let auth = auth.clone();
    let matrix_paths = paths.clone();
    let matrix_allowed = config.matrix_allowed_users.clone();
    let proactive_config = ProactiveConfig {
        session_heartbeat_secs: config.session_heartbeat_secs,
        heartbeat_prompt: config.heartbeat_prompt.clone(),
        trigger_pipe_enabled: config.trigger_pipe_enabled,
    };

    Some(tokio::spawn(async move {
        if let Err(e) = super::matrix_bridge::run_matrix_bridge(
            state,
            registry,
            factory,
            auth,
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
    match iroh::start_endpoint(identity).await {
        Ok(ep) => {
            let hb_endpoint = Arc::new(ep);
            tokio::spawn(iroh::run_heartbeat(hb_endpoint, registry_path, interval, cancel));
        }
        Err(e) => {
            warn!("heartbeat disabled: iroh endpoint unavailable: {e}");
        }
    }
}

fn spawn_status_logger(
    state: Arc<tokio::sync::Mutex<clankers_controller::transport::DaemonState>>,
    cancel: CancellationToken,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let st = state.lock().await;
                    info!("daemon status: {} active session(s)", st.sessions.len());
                }
                () = cancel.cancelled() => break,
            }
        }
    });
}

fn spawn_idle_reaper(
    config: &DaemonConfig,
    state: Arc<tokio::sync::Mutex<clankers_controller::transport::DaemonState>>,
    _registry: clanker_actor::ProcessRegistry,
    catalog: Option<Arc<session_store::SessionCatalog>>,
    cancel: CancellationToken,
) {
    if config.idle_timeout_secs == 0 {
        return;
    }

    let idle_timeout = std::time::Duration::from_secs(config.idle_timeout_secs);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let stale = {
                        let st = state.lock().await;
                        let now = chrono::Utc::now();
                        st.sessions
                            .iter()
                            .filter(|(_, h)| {
                                if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&h.last_active) {
                                    let idle = now.signed_duration_since(ts);
                                    idle.to_std().unwrap_or_default() > idle_timeout
                                } else {
                                    false
                                }
                            })
                            .map(|(id, _)| id.clone())
                            .collect::<Vec<_>>()
                    };

                    if !stale.is_empty() {
                        let mut st = state.lock().await;
                        for session_id in &stale {
                            if let Some(handle) = st.sessions.get(session_id)
                                && let Some(ref tx) = handle.cmd_tx
                            {
                                let _ = tx.send(clankers_protocol::SessionCommand::Disconnect);
                            }
                            st.remove_session(session_id);
                            if let Some(ref catalog) = catalog {
                                catalog.set_state(session_id, session_store::SessionLifecycle::Tombstoned);
                            }
                        }
                        info!("Reaped {} idle session(s)", stale.len());
                    }
                }
                () = cancel.cancelled() => break,
            }
        }
    });
}

/// Periodically GC tombstoned catalog entries older than 7 days.
fn spawn_catalog_gc(
    catalog: Option<Arc<session_store::SessionCatalog>>,
    cancel: CancellationToken,
) {
    let Some(catalog) = catalog else { return };

    tokio::spawn(async move {
        // Run GC every hour
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        let retention = std::time::Duration::from_secs(7 * 24 * 3600); // 7 days
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let removed = catalog.gc_tombstoned(retention);
                    if removed > 0 {
                        info!("Catalog GC: removed {removed} tombstoned entries");
                    }
                }
                () = cancel.cancelled() => break,
            }
        }
    });
}

/// Periodically sync DaemonState metadata to the session catalog (every 5s).
/// Updates `last_active` and `turn_count` for all active sessions.
fn spawn_catalog_updater(
    catalog: Option<Arc<session_store::SessionCatalog>>,
    state: Arc<tokio::sync::Mutex<clankers_controller::transport::DaemonState>>,
    cancel: CancellationToken,
) {
    let Some(catalog) = catalog else { return };

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let st = state.lock().await;
                    for handle in st.sessions.values() {
                        if let Some(mut entry) = catalog.get_session(&handle.session_id) {
                            let changed = entry.last_active != handle.last_active
                                || entry.turn_count != handle.turn_count;
                            if changed {
                                entry.last_active.clone_from(&handle.last_active);
                                entry.turn_count = handle.turn_count;
                                catalog.update_session(&entry);
                            }
                        }
                    }
                }
                () = cancel.cancelled() => break,
            }
        }
    });
}
