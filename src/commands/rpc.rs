//! Peer-to-peer RPC command handlers via iroh.
//!
//! Each RPC subcommand is handled by a focused function. The top-level
//! `run()` dispatcher resolves identity then routes to the correct handler.

use crate::cli::PeerAction;
use crate::cli::RpcAction;
use crate::commands::CommandContext;
use crate::error::Result;

/// Maximum number of parallel probe targets in discovery (prevents unbounded iteration).
const MAX_PROBE_TARGETS: usize = 256;

// ── Dispatcher ──────────────────────────────────────────────────────────────

/// Main RPC command dispatcher.
///
/// Resolves the node identity, then delegates to a per-action handler.
/// Each handler is under 70 lines and does exactly one thing.
pub async fn run(ctx: &CommandContext, identity_path: Option<String>, action: RpcAction) -> Result<()> {
    let identity_path = identity_path
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| crate::modes::rpc::iroh::identity_path(&ctx.paths));
    let identity = crate::modes::rpc::iroh::Identity::load_or_generate(&identity_path);

    match action {
        RpcAction::Id => handle_id(&identity),
        RpcAction::Start {
            with_agent,
            tags,
            allow_all,
            heartbeat,
            heartbeat_interval,
        } => handle_start(ctx, &identity, with_agent, tags, allow_all, heartbeat, heartbeat_interval).await,
        RpcAction::Ping { node_id } => handle_simple_rpc(&identity, &node_id, "ping", true).await,
        RpcAction::Version { node_id } => handle_simple_rpc(&identity, &node_id, "version", false).await,
        RpcAction::Status { node_id } => handle_simple_rpc(&identity, &node_id, "status", false).await,
        RpcAction::Prompt { node_id, text } => handle_prompt(&identity, &node_id, &text).await,
        RpcAction::Peers { action: peer_action } => handle_peers(ctx, &identity, peer_action).await,
        RpcAction::Allow { node_id } => handle_allow(ctx, &node_id),
        RpcAction::Deny { node_id } => handle_deny(ctx, &node_id),
        RpcAction::Allowed => handle_allowed(ctx),
        RpcAction::Discover { mdns, scan_secs } => handle_discover(ctx, &identity, mdns, scan_secs).await,
        RpcAction::SendFile { node_id, file } => handle_send_file(&identity, &node_id, &file).await,
        RpcAction::RecvFile {
            node_id,
            remote_path,
            output,
        } => handle_recv_file(&identity, &node_id, &remote_path, output.as_deref()).await,
    }
}

// ── Identity ────────────────────────────────────────────────────────────────

fn handle_id(identity: &crate::modes::rpc::iroh::Identity) -> Result<()> {
    let pk = identity.public_key();
    println!("Node ID: {}", pk);
    println!("Short:   {}", pk.fmt_short());
    Ok(())
}

// ── Server start ────────────────────────────────────────────────────────────

async fn handle_start(
    ctx: &CommandContext,
    identity: &crate::modes::rpc::iroh::Identity,
    with_agent: bool,
    tags: Vec<String>,
    allow_all: bool,
    heartbeat: bool,
    heartbeat_interval: u64,
) -> Result<()> {
    let endpoint = crate::modes::rpc::iroh::start_endpoint(identity).await?;
    let addr = endpoint.addr();
    println!("RPC server started");
    println!("Node ID: {}", endpoint.id());
    println!("Addr:    {:?}", addr);

    let acl = build_acl(ctx, allow_all);
    println!(
        "Auth: {}",
        if acl.allow_all {
            "open (--allow-all)".to_string()
        } else {
            format!("{} allowed peer(s)", acl.allowed.len())
        }
    );

    let agent_names = discover_agent_names(ctx);
    let agent_ctx = if with_agent {
        Some(build_agent_context(ctx)?)
    } else {
        None
    };

    let receive_dir = ctx.paths.global_config_dir.join("received");
    let state = std::sync::Arc::new(crate::modes::rpc::iroh::ServerState {
        meta: crate::modes::rpc::iroh::NodeMeta {
            tags: tags.clone(),
            agent_names,
        },
        agent: agent_ctx,
        acl,
        receive_dir: Some(receive_dir),
    });

    println!("Agent support: {}", if state.agent.is_some() { "enabled" } else { "disabled" });
    if !tags.is_empty() {
        println!("Tags: {}", tags.join(", "));
    }

    let cancel = tokio_util::sync::CancellationToken::new();
    if heartbeat {
        let registry_path = crate::modes::rpc::peers::registry_path(&ctx.paths);
        let interval = std::time::Duration::from_secs(heartbeat_interval);
        let ep = std::sync::Arc::new(crate::modes::rpc::iroh::start_endpoint(identity).await?);
        println!("Heartbeat: every {}s", heartbeat_interval);
        tokio::spawn(crate::modes::rpc::iroh::run_heartbeat(ep, registry_path, interval, cancel.clone()));
    }

    println!("\nListening... (Ctrl+C to stop)\n");
    println!("Test with:  clankers rpc ping {}", endpoint.id());

    crate::modes::rpc::iroh::serve_rpc(endpoint, state).await?;
    cancel.cancel();
    Ok(())
}

// ── Simple RPC (ping/version/status) ────────────────────────────────────────

/// Sends a single RPC method and prints the result.
///
/// `measure_latency`: if true, prints round-trip time (used by ping).
async fn handle_simple_rpc(
    identity: &crate::modes::rpc::iroh::Identity,
    node_id: &str,
    method: &str,
    measure_latency: bool,
) -> Result<()> {
    let remote = parse_node_id(node_id)?;
    let endpoint = crate::modes::rpc::iroh::start_endpoint(identity).await?;
    let request = crate::modes::rpc::protocol::Request::new(method, serde_json::json!({}));

    if measure_latency {
        println!("Pinging {}...", remote.fmt_short());
    }

    let start = std::time::Instant::now();
    let response = crate::modes::rpc::iroh::send_rpc(&endpoint, remote, &request).await?;
    let elapsed = start.elapsed();

    if let Some(result) = response.ok {
        if measure_latency {
            println!("Response: {} ({}ms)", result, elapsed.as_millis());
        } else {
            println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
        }
        Ok(())
    } else {
        Err(rpc_error(response.error))
    }
}

// ── Streaming prompt ────────────────────────────────────────────────────────

async fn handle_prompt(identity: &crate::modes::rpc::iroh::Identity, node_id: &str, text: &str) -> Result<()> {
    let remote = parse_node_id(node_id)?;
    let endpoint = crate::modes::rpc::iroh::start_endpoint(identity).await?;
    let request = crate::modes::rpc::protocol::Request::new("prompt", serde_json::json!({ "text": text }));
    eprintln!("Sending prompt to {}...", remote.fmt_short());

    let (_notifications, response) =
        crate::modes::rpc::iroh::send_rpc_streaming(&endpoint, remote, &request, |notification| {
            handle_prompt_notification(notification);
        })
        .await?;

    println!(); // newline after streamed text
    if let Some(err) = response.error {
        Err(crate::error::Error::Provider {
            message: format!("RPC error: {}", err),
        })
    } else {
        Ok(())
    }
}

/// Process a streaming prompt notification (text delta, tool call, or tool result).
fn handle_prompt_notification(notification: &serde_json::Value) {
    let method = match notification.get("method").and_then(|v| v.as_str()) {
        Some(m) => m,
        None => return,
    };
    match method {
        "agent.text_delta" => {
            if let Some(text) = notification.get("params").and_then(|p| p.get("text")).and_then(|v| v.as_str()) {
                print!("{}", text);
                use std::io::Write;
                std::io::stdout().flush().ok();
            }
        }
        "agent.tool_call" => {
            if let Some(params) = notification.get("params") {
                let tool = params.get("tool_name").and_then(|v| v.as_str()).unwrap_or("?");
                eprintln!("\n[tool: {}]", tool);
            }
        }
        "agent.tool_result" => {
            if let Some(params) = notification.get("params") {
                let is_error = params.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false);
                if is_error {
                    eprintln!("[tool error]");
                }
            }
        }
        _ => {}
    }
}

// ── Peer management ─────────────────────────────────────────────────────────

async fn handle_peers(
    ctx: &CommandContext,
    identity: &crate::modes::rpc::iroh::Identity,
    peer_action: PeerAction,
) -> Result<()> {
    let registry_path = crate::modes::rpc::peers::registry_path(&ctx.paths);
    let mut registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);

    match peer_action {
        PeerAction::List => {
            print_peer_list(&registry);
            Ok(())
        }
        PeerAction::Add { node_id, name } => {
            let _: iroh::PublicKey = parse_node_id(&node_id)?;
            registry.add(&node_id, &name);
            registry.save(&registry_path).map_err(|e| crate::error::Error::Io { source: e })?;
            println!("Added peer '{}' ({})", name, truncate_id(&node_id));
            Ok(())
        }
        PeerAction::Remove { peer } => remove_peer(&mut registry, &registry_path, &peer),
        PeerAction::Probe { peer } => probe_peers(identity, &mut registry, &registry_path, &peer).await,
    }
}

fn print_peer_list(registry: &crate::modes::rpc::peers::PeerRegistry) {
    let peers = registry.list();
    if peers.is_empty() {
        println!("No known peers. Add one with: clankers rpc peers add <node-id> <name>");
        return;
    }
    for peer in peers {
        let seen = peer
            .last_seen
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "never".to_string());
        let caps = if peer.capabilities.accepts_prompts {
            "✓ prompts"
        } else {
            "✗ prompts"
        };
        let tags = if peer.capabilities.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", peer.capabilities.tags.join(", "))
        };
        let agents = if peer.capabilities.agents.is_empty() {
            String::new()
        } else {
            format!(" agents: {}", peer.capabilities.agents.join(", "))
        };
        println!(
            "  {} ({}) — {} | last seen: {}{}{}",
            peer.name,
            truncate_id(&peer.node_id),
            caps,
            seen,
            tags,
            agents,
        );
    }
}

fn remove_peer(
    registry: &mut crate::modes::rpc::peers::PeerRegistry,
    registry_path: &std::path::Path,
    peer: &str,
) -> Result<()> {
    let removed = if registry.remove(peer) {
        true
    } else {
        // Search by name
        let found = registry.peers.values().find(|p| p.name == peer).map(|p| p.node_id.clone());
        if let Some(nid) = found {
            registry.remove(&nid)
        } else {
            false
        }
    };
    if removed {
        registry.save(registry_path).map_err(|e| crate::error::Error::Io { source: e })?;
        println!("Removed peer '{}'", peer);
        Ok(())
    } else {
        Err(crate::error::Error::Config {
            message: format!("Peer '{}' not found", peer),
        })
    }
}

async fn probe_peers(
    identity: &crate::modes::rpc::iroh::Identity,
    registry: &mut crate::modes::rpc::peers::PeerRegistry,
    registry_path: &std::path::Path,
    peer: &str,
) -> Result<()> {
    let endpoint = crate::modes::rpc::iroh::start_endpoint(identity).await?;

    let targets: Vec<(String, String)> = if peer == "all" {
        registry
            .list()
            .iter()
            .take(MAX_PROBE_TARGETS)
            .map(|p| (p.node_id.clone(), p.name.clone()))
            .collect()
    } else {
        let (node_id, name) = resolve_peer_target(registry, peer);
        vec![(node_id, name)]
    };

    for (node_id, name) in &targets {
        let remote: iroh::PublicKey = match node_id.parse() {
            Ok(pk) => pk,
            Err(e) => {
                eprintln!("  {} — invalid node ID: {}", name, e);
                continue;
            }
        };
        print!("  Probing {}... ", name);
        let request = crate::modes::rpc::protocol::Request::new("status", serde_json::json!({}));
        match crate::modes::rpc::iroh::send_rpc(&endpoint, remote, &request).await {
            Ok(response) => {
                if let Some(result) = response.ok {
                    let caps = parse_capabilities(&result);
                    let prompt_status = if caps.accepts_prompts { "✓" } else { "✗" };
                    println!(
                        "online {} prompts | {} tools | tags: [{}]",
                        prompt_status,
                        caps.tools.len(),
                        caps.tags.join(", "),
                    );
                    registry.update_capabilities(node_id, caps);
                } else {
                    println!("online (no status data)");
                    registry.touch(node_id);
                }
            }
            Err(e) => {
                println!("offline ({})", e);
            }
        }
    }
    registry.save(registry_path).map_err(|e| crate::error::Error::Io { source: e })?;
    Ok(())
}

// ── ACL management ──────────────────────────────────────────────────────────

fn handle_allow(ctx: &CommandContext, node_id: &str) -> Result<()> {
    let _: iroh::PublicKey = parse_node_id(node_id)?;
    let acl_path = crate::modes::rpc::iroh::allowlist_path(&ctx.paths);
    let mut allowed = crate::modes::rpc::iroh::load_allowlist(&acl_path);
    allowed.insert(node_id.to_string());
    crate::modes::rpc::iroh::save_allowlist(&acl_path, &allowed).map_err(|e| crate::error::Error::Io { source: e })?;
    println!("Allowed peer: {}", truncate_id(node_id));
    println!("Total allowed: {}", allowed.len());
    Ok(())
}

fn handle_deny(ctx: &CommandContext, node_id: &str) -> Result<()> {
    let acl_path = crate::modes::rpc::iroh::allowlist_path(&ctx.paths);
    let mut allowed = crate::modes::rpc::iroh::load_allowlist(&acl_path);
    if allowed.remove(node_id) {
        crate::modes::rpc::iroh::save_allowlist(&acl_path, &allowed)
            .map_err(|e| crate::error::Error::Io { source: e })?;
        println!("Denied peer: {}", truncate_id(node_id));
        Ok(())
    } else {
        Err(crate::error::Error::Config {
            message: "Peer not in allowlist".to_string(),
        })
    }
}

fn handle_allowed(ctx: &CommandContext) -> Result<()> {
    let acl_path = crate::modes::rpc::iroh::allowlist_path(&ctx.paths);
    let allowed = crate::modes::rpc::iroh::load_allowlist(&acl_path);
    if allowed.is_empty() {
        println!("No peers in allowlist. Use: clankers rpc allow <node-id>");
        println!("Or start server with --allow-all");
    } else {
        println!("Allowed peers ({}):", allowed.len());
        for nid in &allowed {
            println!("  {}", nid);
        }
    }
    Ok(())
}

// ── Discovery ───────────────────────────────────────────────────────────────

async fn handle_discover(
    ctx: &CommandContext,
    identity: &crate::modes::rpc::iroh::Identity,
    mdns: bool,
    scan_secs: u64,
) -> Result<()> {
    let registry_path = crate::modes::rpc::peers::registry_path(&ctx.paths);
    let mut registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
    let endpoint = crate::modes::rpc::iroh::start_endpoint(identity).await?;

    if mdns {
        discover_mdns_peers(&endpoint, &mut registry, &registry_path, scan_secs).await?;
    }

    let peers: Vec<(String, String)> = registry
        .list()
        .iter()
        .take(MAX_PROBE_TARGETS)
        .map(|p| (p.node_id.clone(), p.name.clone()))
        .collect();

    if peers.is_empty() {
        println!("No known peers. Add some with: clankers rpc peers add <node-id> <name>");
        println!("Or use --mdns to scan the local network.");
        return Ok(());
    }

    println!("Probing {} peer(s)...\n", peers.len());

    let mut online: u32 = 0;
    for (node_id, name) in &peers {
        let remote: iroh::PublicKey = match node_id.parse() {
            Ok(pk) => pk,
            Err(_) => {
                println!("  {} — invalid node ID", name);
                continue;
            }
        };
        let request = crate::modes::rpc::protocol::Request::new("status", serde_json::json!({}));
        match tokio::time::timeout(
            std::time::Duration::from_secs(10),
            crate::modes::rpc::iroh::send_rpc(&endpoint, remote, &request),
        )
        .await
        {
            Ok(Ok(response)) => {
                online = online.saturating_add(1);
                print_probe_result(&mut registry, node_id, name, response.ok);
            }
            Ok(Err(_)) | Err(_) => {
                println!("  ○ {} ({}) — offline", name, truncate_id(node_id));
            }
        }
    }
    registry.save(&registry_path).map_err(|e| crate::error::Error::Io { source: e })?;
    println!("\n{}/{} peers online", online, peers.len());
    Ok(())
}

async fn discover_mdns_peers(
    endpoint: &iroh::Endpoint,
    registry: &mut crate::modes::rpc::peers::PeerRegistry,
    registry_path: &std::path::Path,
    scan_secs: u64,
) -> Result<()> {
    let duration = std::time::Duration::from_secs(scan_secs);
    let discovered = crate::modes::rpc::iroh::discover_mdns_peers(endpoint, duration).await;

    if discovered.is_empty() {
        println!("No new peers found via mDNS.");
    } else {
        println!("mDNS discovered {} peer(s):", discovered.len());
        for (eid, _addr) in discovered.iter().take(MAX_PROBE_TARGETS) {
            let nid = eid.to_string();
            let short = truncate_id(&nid);
            if !registry.peers.contains_key(&nid) {
                registry.add(&nid, &format!("mdns-{}", short));
                println!("  + {} (auto-added as mdns-{})", short, short);
            } else {
                println!("  = {} (already known)", short);
            }
        }
        registry.save(registry_path).map_err(|e| crate::error::Error::Io { source: e })?;
        println!();
    }
    Ok(())
}

fn print_probe_result(
    registry: &mut crate::modes::rpc::peers::PeerRegistry,
    node_id: &str,
    name: &str,
    result: Option<serde_json::Value>,
) {
    if let Some(result) = result {
        let caps = parse_capabilities(&result);
        let prompt_icon = if caps.accepts_prompts { "✓" } else { "✗" };
        let tags_str = if caps.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", caps.tags.join(", "))
        };
        println!("  ● {} ({}) — {} prompts{}", name, truncate_id(node_id), prompt_icon, tags_str);
        registry.update_capabilities(node_id, caps);
    } else {
        println!("  ● {} — online", name);
        registry.touch(node_id);
    }
}

// ── File transfer ───────────────────────────────────────────────────────────

async fn handle_send_file(identity: &crate::modes::rpc::iroh::Identity, node_id: &str, file: &str) -> Result<()> {
    let remote = parse_node_id(node_id)?;
    let file_path = std::path::Path::new(file);
    if !file_path.exists() {
        return Err(crate::error::Error::Config {
            message: format!("File not found: {}", file),
        });
    }
    let endpoint = crate::modes::rpc::iroh::start_endpoint(identity).await?;
    let file_size = std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
    println!("Sending '{}' ({} bytes) to {}...", file_path.display(), file_size, remote.fmt_short());
    let response = crate::modes::rpc::iroh::send_file(&endpoint, remote, file_path).await?;
    if let Some(result) = response.ok {
        let remote_path = result.get("path").and_then(|v| v.as_str()).unwrap_or("?");
        let size = result.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
        println!("✓ Sent {} bytes → {}", size, remote_path);
        Ok(())
    } else {
        Err(rpc_error(response.error))
    }
}

async fn handle_recv_file(
    identity: &crate::modes::rpc::iroh::Identity,
    node_id: &str,
    remote_path: &str,
    output: Option<&str>,
) -> Result<()> {
    let remote = parse_node_id(node_id)?;
    let local_path = match output {
        Some(p) => std::path::PathBuf::from(p),
        None => {
            let name = std::path::Path::new(remote_path).file_name().and_then(|n| n.to_str()).unwrap_or("downloaded");
            std::path::PathBuf::from(name)
        }
    };
    let endpoint = crate::modes::rpc::iroh::start_endpoint(identity).await?;
    println!("Downloading '{}' from {} → {}...", remote_path, remote.fmt_short(), local_path.display());
    let total = crate::modes::rpc::iroh::recv_file(&endpoint, remote, remote_path, &local_path).await?;
    println!("✓ Received {} bytes → {}", total, local_path.display());
    Ok(())
}

// ── Shared helpers ──────────────────────────────────────────────────────────

/// Parse a node ID string into an iroh PublicKey.
fn parse_node_id(node_id: &str) -> Result<iroh::PublicKey> {
    node_id.parse().map_err(|e| crate::error::Error::Config {
        message: format!("Invalid node ID: {}", e),
    })
}

/// Truncate a node ID for display (first 12 chars).
fn truncate_id(node_id: &str) -> &str {
    &node_id[..12.min(node_id.len())]
}

/// Convert an optional error string into a Provider error.
fn rpc_error(error: Option<String>) -> crate::error::Error {
    crate::error::Error::Provider {
        message: error.map(|e| format!("RPC error: {}", e)).unwrap_or_else(|| "No response from peer".to_string()),
    }
}

/// Build an ACL from the allowlist or --allow-all flag.
fn build_acl(ctx: &CommandContext, allow_all: bool) -> crate::modes::rpc::iroh::AccessControl {
    let acl_path = crate::modes::rpc::iroh::allowlist_path(&ctx.paths);
    if allow_all {
        crate::modes::rpc::iroh::AccessControl::open()
    } else {
        let allowed = crate::modes::rpc::iroh::load_allowlist(&acl_path);
        if allowed.is_empty() {
            println!("WARNING: allowlist is empty — no peers can connect.");
            println!("  Use --allow-all, or add peers with: clankers rpc allow <node-id>");
        }
        crate::modes::rpc::iroh::AccessControl::from_allowlist(allowed)
    }
}

/// Discover agent definitions and return their names.
fn discover_agent_names(ctx: &CommandContext) -> Vec<String> {
    let agent_scope = crate::agent_defs::definition::AgentScope::default();
    let agent_registry = crate::agent_defs::discovery::discover_agents(
        &ctx.paths.global_agents_dir,
        Some(&ctx.project_paths.agents_dir),
        &agent_scope,
    );
    agent_registry.list().iter().map(|a| a.name.clone()).collect()
}

/// Build the agent context for RPC server (provider + tools).
fn build_agent_context(ctx: &CommandContext) -> Result<crate::modes::rpc::iroh::RpcContext> {
    let provider = crate::provider::discovery::build_router(
        ctx.api_key.as_deref(),
        ctx.api_base.clone(),
        &ctx.paths.global_auth,
        ctx.paths.pi_auth.as_deref(),
        ctx.account.as_deref(),
    )?;
    let rpc_process_monitor = {
        let config = crate::procmon::ProcessMonitorConfig::default();
        let monitor = std::sync::Arc::new(crate::procmon::ProcessMonitor::new(config, None));
        monitor.clone().start();
        monitor
    };
    let env = crate::modes::common::ToolEnv {
        process_monitor: Some(rpc_process_monitor),
        ..Default::default()
    };
    // RPC mode: all tiers active
    let tiered = crate::modes::common::build_tiered_tools(&env);
    let tool_set = crate::modes::common::ToolSet::new(tiered, [
        crate::modes::common::ToolTier::Core,
        crate::modes::common::ToolTier::Orchestration,
        crate::modes::common::ToolTier::Specialty,
        crate::modes::common::ToolTier::Matrix,
    ]);
    let tools = tool_set.active_tools();
    Ok(crate::modes::rpc::iroh::RpcContext {
        provider,
        tools,
        settings: ctx.settings.clone(),
        model: ctx.model.clone(),
        system_prompt: ctx.system_prompt.clone(),
    })
}

/// Parse capabilities from a JSON status response.
fn parse_capabilities(result: &serde_json::Value) -> crate::modes::rpc::peers::PeerCapabilities {
    let json_str_vec = |key: &str| -> Vec<String> {
        result
            .get(key)
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    };

    crate::modes::rpc::peers::PeerCapabilities {
        accepts_prompts: result.get("accepts_prompts").and_then(|v| v.as_bool()).unwrap_or(false),
        agents: json_str_vec("agents"),
        tools: json_str_vec("tools"),
        tags: json_str_vec("tags"),
        version: result.get("version").and_then(|v| v.as_str()).map(String::from),
    }
}

/// Resolve a peer identifier (node_id or name) to (node_id, display_name).
fn resolve_peer_target(registry: &crate::modes::rpc::peers::PeerRegistry, peer: &str) -> (String, String) {
    if let Some(_p) = registry.peers.get(peer) {
        let name = registry.peers.get(peer).map(|p| p.name.clone()).unwrap_or_else(|| truncate_id(peer).to_string());
        (peer.to_string(), name)
    } else if let Some(p) = registry.peers.values().find(|p| p.name == peer) {
        (p.node_id.clone(), p.name.clone())
    } else {
        // Treat as raw node_id not in registry
        (peer.to_string(), truncate_id(peer).to_string())
    }
}
