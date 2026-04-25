//! Embedded RPC server started alongside the TUI for swarm presence.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use tokio_util::sync::CancellationToken;

use crate::config::Settings;
use crate::error::Result;
use crate::tui::app::App;

/// Configuration for the embedded RPC server that runs inside the TUI process.
pub struct EmbeddedRpcConfig {
    /// Capability tags to advertise
    pub tags: Vec<String>,
    /// Whether to accept prompts from remote peers
    pub with_agent: bool,
    /// Whether to allow all peers (no allowlist)
    pub allow_all: bool,
    /// Heartbeat interval (None = disabled)
    pub heartbeat_interval: Option<std::time::Duration>,
}

impl Default for EmbeddedRpcConfig {
    fn default() -> Self {
        Self {
            tags: Vec::new(),
            with_agent: false,
            allow_all: true,
            heartbeat_interval: Some(std::time::Duration::from_secs(120)),
        }
    }
}

/// Start the embedded RPC server in the background. Returns the node's
/// public key (EndpointId) and a cancellation token to shut it down.
///
/// The server shares the same process as the TUI but runs on a separate
/// tokio task. It advertises this node via mDNS for LAN discovery and
/// optionally runs a heartbeat to probe known peers.
pub async fn start_embedded_rpc(
    config: EmbeddedRpcConfig,
    provider: Option<std::sync::Arc<dyn crate::provider::Provider>>,
    tools: Vec<std::sync::Arc<dyn crate::tools::Tool>>,
    settings: crate::config::settings::Settings,
    model: String,
    system_prompt: String,
) -> Result<(String, CancellationToken)> {
    use crate::modes::rpc::iroh;

    let paths = crate::config::ClankersPaths::get();
    let identity_path = iroh::identity_path(paths);
    let identity = iroh::Identity::load_or_generate(&identity_path);
    let node_id = identity.public_key().to_string();

    let endpoint = iroh::start_endpoint(&identity).await?;

    // Build ACL
    let acl = if config.allow_all {
        iroh::AccessControl::open()
    } else {
        let acl_path = iroh::allowlist_path(paths);
        let allowed = iroh::load_allowlist(&acl_path);
        iroh::AccessControl::from_allowlist(allowed)
    };

    // Build agent context if requested
    let agent_ctx = if config.with_agent {
        provider.map(|p| iroh::RpcContext {
            provider: p,
            tools,
            settings: settings.clone(),
            model: model.clone(),
            system_prompt: system_prompt.clone(),
        })
    } else {
        None
    };

    let state = std::sync::Arc::new(iroh::ServerState {
        meta: iroh::NodeMeta {
            tags: config.tags,
            agent_names: Vec::new(),
        },
        agent: agent_ctx,
        acl,
        receive_dir: None,
    });

    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    // Start the RPC server
    let endpoint_for_serve = endpoint.clone();
    tokio::spawn(async move {
        tokio::select! {
            result = iroh::serve_rpc(endpoint_for_serve, state) => {
                if let Err(e) = result {
                    tracing::warn!("Embedded RPC server error: {}", e);
                }
            }
            () = cancel_clone.cancelled() => {
                tracing::info!("Embedded RPC server shut down");
            }
        }
    });

    // Start heartbeat if configured
    if let Some(interval) = config.heartbeat_interval {
        let registry_path = crate::modes::rpc::peers::registry_path(paths);
        let heartbeat_cancel = cancel.clone();
        let endpoint_arc = std::sync::Arc::new(endpoint);
        tokio::spawn(iroh::run_heartbeat(endpoint_arc, registry_path, interval, heartbeat_cancel));
    }

    tracing::info!("Embedded RPC server started as {}", &node_id[..12.min(node_id.len())]);
    Ok((node_id, cancel))
}

/// Try to start the embedded RPC server for swarm presence.
///
/// Makes this instance discoverable on the LAN via mDNS. Skipped in test
/// environments or when `CLANKERS_NO_RPC` is set. Returns a cancellation
/// token if the server started successfully.
pub(super) async fn maybe_start_rpc(app: &mut App, paths: &crate::config::ClankersPaths) -> Option<CancellationToken> {
    if cfg!(test) || std::env::var("CLANKERS_NO_RPC").is_ok() {
        return None;
    }

    let config = EmbeddedRpcConfig {
        tags: vec![],
        with_agent: false,
        allow_all: true,
        heartbeat_interval: Some(std::time::Duration::from_secs(120)),
    };

    match start_embedded_rpc(config, None, Vec::new(), Settings::default(), String::new(), String::new()).await {
        Ok((node_id, cancel)) => {
            let short_id = if node_id.len() > 12 {
                format!("{}…", &node_id[..12])
            } else {
                node_id.clone()
            };
            let pp = peers_panel(app);
            pp.self_id = Some(short_id);
            pp.server_running = true;
            let registry =
                crate::modes::rpc::peers::PeerRegistry::load(&crate::modes::rpc::peers::registry_path(paths));
            let entries = crate::tui::components::peers_panel::entries_from_registry(
                &crate::modes::rpc::peers::peer_info_views(&registry),
                chrono::Duration::minutes(5),
            );
            pp.set_peers(entries);
            Some(cancel)
        }
        Err(e) => {
            tracing::debug!("Embedded RPC not available: {}", e);
            None
        }
    }
}

/// Helper to access the PeersPanel.
#[cfg_attr(dylint_lib = "tigerstyle", allow(no_unwrap, reason = "panel registered at startup"))]
fn peers_panel(app: &mut App) -> &mut crate::tui::components::peers_panel::PeersPanel {
    app.panels
        .downcast_mut::<crate::tui::components::peers_panel::PeersPanel>(crate::tui::panel::PanelId::Peers)
        .expect("peers panel registered at startup")
}
