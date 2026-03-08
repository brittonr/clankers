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
use tracing::{error, info, warn};

use crate::config::ClankersPaths;
use crate::error::Result;
use crate::modes::rpc::iroh;
use crate::provider::Provider;
use crate::tools::Tool;

mod config;
mod handlers;
mod session_store;

// Re-export public types
pub use config::DaemonConfig;
pub(crate) use config::{ProactiveConfig, ALPN_CHAT};
pub(crate) use session_store::{SessionKey, SessionStore};

use handlers::handle_iroh_connection;
use session_store::create_auth_layer;

// ── Daemon entry point ──────────────────────────────────────────────────────

/// Start the daemon. Blocks until cancelled.
pub async fn run_daemon(
    provider: Arc<dyn Provider>,
    tools: Vec<Arc<dyn Tool>>,
    config: DaemonConfig,
    paths: &ClankersPaths,
) -> Result<()> {
    let cancel = CancellationToken::new();

    // ── iroh identity (needed for auth layer trusted root) ──────────
    let identity_path = iroh::identity_path(paths);
    let identity = iroh::Identity::load_or_generate(&identity_path);

    // ── Auth layer (UCAN tokens) ───────────────────────────────────
    let db_path = paths.global_config_dir.join("clankers.db");
    let auth_layer = create_auth_layer(&db_path, &identity);

    // Build the session store
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

    // ── iroh endpoint ───────────────────────────────────────────────
    let node_id = identity.public_key();

    // Build endpoint that accepts both ALPNs
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

    // Build ACL
    let acl_path = iroh::allowlist_path(paths);
    let acl = if config.allow_all {
        iroh::AccessControl::open()
    } else {
        let allowed = iroh::load_allowlist(&acl_path);
        iroh::AccessControl::from_allowlist(allowed)
    };
    let acl = Arc::new(acl);

    println!("clankers daemon started");
    println!("  Node ID:  {}", node_id);
    println!("  Auth:     {}", if config.allow_all { "open" } else { "allowlist + UCAN tokens" });
    println!("  Model:    {}", config.model);
    println!("  Sessions: 0/{}", config.max_sessions);
    if !config.tags.is_empty() {
        println!("  Tags:     {}", config.tags.join(", "));
    }
    println!("  Tokens:   create with `clankers token create`");

    // ── iroh accept loop ────────────────────────────────────────────
    let iroh_store = Arc::clone(&store);
    let iroh_acl = Arc::clone(&acl);
    let iroh_cancel = cancel.clone();

    // Also build the legacy RPC state for the rpc/1 ALPN
    let rpc_state = Arc::new(iroh::ServerState {
        meta: iroh::NodeMeta {
            tags: config.tags.clone(),
            agent_names: Vec::new(),
        },
        agent: Some(iroh::RpcContext {
            provider: Arc::clone(&provider),
            tools: tools.clone(),
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
    });

    let iroh_endpoint = endpoint.clone();
    let iroh_handle = tokio::spawn(async move {
        info!("iroh accept loop started");
        loop {
            tokio::select! {
                incoming = iroh_endpoint.accept() => {
                    let Some(incoming) = incoming else { break };
                    let store = Arc::clone(&iroh_store);
                    let acl = Arc::clone(&iroh_acl);
                    let rpc_state = Arc::clone(&rpc_state);

                    tokio::spawn(async move {
                        if let Err(e) = handle_iroh_connection(incoming, store, acl, rpc_state).await {
                            warn!("iroh connection error: {e}");
                        }
                    });
                }
                () = iroh_cancel.cancelled() => break,
            }
        }
    });

    // ── Matrix bridge (optional) ────────────────────────────────────
    let matrix_handle = if config.enable_matrix {
        let matrix_store = Arc::clone(&store);
        let matrix_cancel = cancel.clone();
        let matrix_paths = paths.clone();
        let matrix_allowed = config.matrix_allowed_users.clone();
        let proactive_config = ProactiveConfig {
            session_heartbeat_secs: config.session_heartbeat_secs,
            heartbeat_prompt: config.heartbeat_prompt.clone(),
            trigger_pipe_enabled: config.trigger_pipe_enabled,
        };
        Some(tokio::spawn(async move {
            if let Err(e) =
                super::matrix_bridge::run_matrix_bridge(matrix_store, matrix_cancel, &matrix_paths, matrix_allowed, proactive_config).await
            {
                error!("Matrix bridge error: {e}");
            }
        }))
    } else {
        None
    };

    // ── Heartbeat ───────────────────────────────────────────────────
    if config.heartbeat_secs > 0 {
        let registry_path = crate::modes::rpc::peers::registry_path(paths);
        let interval = std::time::Duration::from_secs(config.heartbeat_secs);
        let hb_endpoint = Arc::new(
            iroh::start_endpoint(&identity)
                .await
                .unwrap_or_else(|_| panic!("failed to start heartbeat endpoint")),
        );
        let hb_cancel = cancel.clone();
        tokio::spawn(iroh::run_heartbeat(hb_endpoint, registry_path, interval, hb_cancel));
    }

    // ── Status logging ──────────────────────────────────────────────
    let status_store = Arc::clone(&store);
    let status_cancel = cancel.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let store = status_store.read().await;
                    info!("daemon status: {} active session(s)", store.len());
                }
                () = status_cancel.cancelled() => break,
            }
        }
    });

    // ── Idle session reaper ─────────────────────────────────────────
    if config.idle_timeout_secs > 0 {
        let reaper_store = Arc::clone(&store);
        let reaper_cancel = cancel.clone();
        let idle_timeout = std::time::Duration::from_secs(config.idle_timeout_secs);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let reaped = reaper_store.write().await.reap_idle(idle_timeout);
                        if reaped > 0 {
                            info!("Reaped {} idle session(s)", reaped);
                        }
                    }
                    () = reaper_cancel.cancelled() => break,
                }
            }
        });
    }

    println!("\nListening... (Ctrl+C to stop)\n");
    println!("Chat:  clankers rpc prompt {} \"hello\"", node_id);
    println!("Ping:  clankers rpc ping {}", node_id);

    // ── Wait for shutdown ───────────────────────────────────────────
    tokio::signal::ctrl_c().await.ok();
    println!("\nShutting down...");
    cancel.cancel();

    iroh_handle.await.ok();
    if let Some(h) = matrix_handle {
        h.await.ok();
    }

    let store = store.read().await;
    println!("Daemon stopped ({} sessions served).", store.len());
    Ok(())
}
