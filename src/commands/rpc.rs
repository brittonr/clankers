//! Peer-to-peer RPC command handlers via iroh.

use crate::cli::PeerAction;
use crate::cli::RpcAction;
use crate::commands::CommandContext;
use crate::error::Result;

/// Main RPC command dispatcher.
///
/// Handles all RPC-related subcommands: id, start, ping, version, status,
/// prompt, peers, allow/deny, discover, send-file, recv-file.
pub async fn run(ctx: &CommandContext, identity_path: Option<String>, action: RpcAction) -> Result<()> {
    let identity_path = identity_path
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| crate::modes::rpc::iroh::identity_path(&ctx.paths));
    let identity = crate::modes::rpc::iroh::Identity::load_or_generate(&identity_path);

    match action {
        RpcAction::Id => {
            let pk = identity.public_key();
            println!("Node ID: {}", pk);
            println!("Short:   {}", pk.fmt_short());
            Ok(())
        }
        RpcAction::Start {
            with_agent,
            tags,
            allow_all,
            heartbeat,
            heartbeat_interval,
        } => {
            let endpoint = crate::modes::rpc::iroh::start_endpoint(&identity).await?;
            let addr = endpoint.addr();
            println!("RPC server started");
            println!("Node ID: {}", endpoint.id());
            println!("Addr:    {:?}", addr);

            // Build ACL
            let acl_path = crate::modes::rpc::iroh::allowlist_path(&ctx.paths);
            let acl = if allow_all {
                crate::modes::rpc::iroh::AccessControl::open()
            } else {
                let allowed = crate::modes::rpc::iroh::load_allowlist(&acl_path);
                if allowed.is_empty() {
                    println!("WARNING: allowlist is empty — no peers can connect.");
                    println!("  Use --allow-all, or add peers with: clankers rpc allow <node-id>");
                }
                crate::modes::rpc::iroh::AccessControl::from_allowlist(allowed)
            };
            println!(
                "Auth: {}",
                if acl.allow_all {
                    "open (--allow-all)".to_string()
                } else {
                    format!("{} allowed peer(s)", acl.allowed.len())
                }
            );

            // Discover available agent definitions
            let agent_scope = crate::agent_defs::definition::AgentScope::default();
            let agent_registry = crate::agent_defs::discovery::discover_agents(
                &ctx.paths.global_agents_dir,
                Some(&ctx.project_paths.agents_dir),
                &agent_scope,
            );
            let agent_names: Vec<String> = agent_registry.list().iter().map(|a| a.name.clone()).collect();

            let agent_ctx = if with_agent {
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
                let tools = crate::modes::common::build_tools_with_env(&env);
                Some(crate::modes::rpc::iroh::RpcContext {
                    provider,
                    tools,
                    settings: ctx.settings.clone(),
                    model: ctx.model.clone(),
                    system_prompt: ctx.system_prompt.clone(),
                })
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

            // Start background heartbeat if requested
            let cancel = tokio_util::sync::CancellationToken::new();
            if heartbeat {
                let registry_path = crate::modes::rpc::peers::registry_path(&ctx.paths);
                let interval = std::time::Duration::from_secs(heartbeat_interval);
                let ep = std::sync::Arc::new(crate::modes::rpc::iroh::start_endpoint(&identity).await?);
                println!("Heartbeat: every {}s", heartbeat_interval);
                tokio::spawn(crate::modes::rpc::iroh::run_heartbeat(ep, registry_path, interval, cancel.clone()));
            }

            println!("\nListening... (Ctrl+C to stop)\n");
            println!("Test with:  clankers rpc ping {}", endpoint.id());

            crate::modes::rpc::iroh::serve_rpc(endpoint, state).await?;
            cancel.cancel(); // Stop heartbeat on shutdown
            Ok(())
        }
        RpcAction::Ping { node_id } => {
            let remote: iroh::PublicKey = node_id.parse().map_err(|e| crate::error::Error::Config {
                message: format!("Invalid node ID: {}", e),
            })?;
            let endpoint = crate::modes::rpc::iroh::start_endpoint(&identity).await?;
            let request = crate::modes::rpc::protocol::Request::new("ping", serde_json::json!({}));
            println!("Pinging {}...", remote.fmt_short());
            let start = std::time::Instant::now();
            let response = crate::modes::rpc::iroh::send_rpc(&endpoint, remote, &request).await?;
            let elapsed = start.elapsed();
            if let Some(result) = response.ok {
                println!("Response: {} ({}ms)", result, elapsed.as_millis());
                Ok(())
            } else if let Some(err) = response.error {
                Err(crate::error::Error::Provider {
                    message: format!("RPC error: {}", err),
                })
            } else {
                Err(crate::error::Error::Provider {
                    message: "No response from peer".to_string(),
                })
            }
        }
        RpcAction::Version { node_id } => {
            let remote: iroh::PublicKey = node_id.parse().map_err(|e| crate::error::Error::Config {
                message: format!("Invalid node ID: {}", e),
            })?;
            let endpoint = crate::modes::rpc::iroh::start_endpoint(&identity).await?;
            let request = crate::modes::rpc::protocol::Request::new("version", serde_json::json!({}));
            let response = crate::modes::rpc::iroh::send_rpc(&endpoint, remote, &request).await?;
            if let Some(result) = response.ok {
                println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
                Ok(())
            } else if let Some(err) = response.error {
                Err(crate::error::Error::Provider {
                    message: format!("RPC error: {}", err),
                })
            } else {
                Err(crate::error::Error::Provider {
                    message: "No response from peer".to_string(),
                })
            }
        }
        RpcAction::Status { node_id } => {
            let remote: iroh::PublicKey = node_id.parse().map_err(|e| crate::error::Error::Config {
                message: format!("Invalid node ID: {}", e),
            })?;
            let endpoint = crate::modes::rpc::iroh::start_endpoint(&identity).await?;
            let request = crate::modes::rpc::protocol::Request::new("status", serde_json::json!({}));
            let response = crate::modes::rpc::iroh::send_rpc(&endpoint, remote, &request).await?;
            if let Some(result) = response.ok {
                println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
                Ok(())
            } else if let Some(err) = response.error {
                Err(crate::error::Error::Provider {
                    message: format!("RPC error: {}", err),
                })
            } else {
                Err(crate::error::Error::Provider {
                    message: "No response from peer".to_string(),
                })
            }
        }
        RpcAction::Prompt { node_id, text } => {
            let remote: iroh::PublicKey = node_id.parse().map_err(|e| crate::error::Error::Config {
                message: format!("Invalid node ID: {}", e),
            })?;
            let endpoint = crate::modes::rpc::iroh::start_endpoint(&identity).await?;
            let request = crate::modes::rpc::protocol::Request::new("prompt", serde_json::json!({ "text": text }));
            eprintln!("Sending prompt to {}...", remote.fmt_short());

            // Use streaming RPC — print text deltas as they arrive
            let (_notifications, response) =
                crate::modes::rpc::iroh::send_rpc_streaming(&endpoint, remote, &request, |notification| {
                    if let Some(method) = notification.get("method").and_then(|v| v.as_str()) {
                        match method {
                            "agent.text_delta" => {
                                if let Some(text) =
                                    notification.get("params").and_then(|p| p.get("text")).and_then(|v| v.as_str())
                                {
                                    print!("{}", text);
                                    use std::io::Write;
                                    let _ = std::io::stdout().flush();
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
        RpcAction::Peers { action: peer_action } => {
            let registry_path = crate::modes::rpc::peers::registry_path(&ctx.paths);
            let mut registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);

            match peer_action {
                PeerAction::List => {
                    let peers = registry.list();
                    if peers.is_empty() {
                        println!("No known peers. Add one with: clankers rpc peers add <node-id> <name>");
                    } else {
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
                                &peer.node_id[..12.min(peer.node_id.len())],
                                caps,
                                seen,
                                tags,
                                agents,
                            );
                        }
                    }
                    Ok(())
                }
                PeerAction::Add { node_id, name } => {
                    // Validate node_id format
                    let _: iroh::PublicKey = node_id.parse().map_err(|e| crate::error::Error::Config {
                        message: format!("Invalid node ID: {}", e),
                    })?;
                    registry.add(&node_id, &name);
                    registry.save(&registry_path).map_err(|e| crate::error::Error::Io { source: e })?;
                    println!("Added peer '{}' ({})", name, &node_id[..12.min(node_id.len())]);
                    Ok(())
                }
                PeerAction::Remove { peer } => {
                    // Try as node_id first, then as name
                    let removed = if registry.remove(&peer) {
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
                        registry.save(&registry_path).map_err(|e| crate::error::Error::Io { source: e })?;
                        println!("Removed peer '{}'", peer);
                        Ok(())
                    } else {
                        Err(crate::error::Error::Config {
                            message: format!("Peer '{}' not found", peer),
                        })
                    }
                }
                PeerAction::Probe { peer } => {
                    let endpoint = crate::modes::rpc::iroh::start_endpoint(&identity).await?;

                    let targets: Vec<(String, String)> = if peer == "all" {
                        registry.list().iter().map(|p| (p.node_id.clone(), p.name.clone())).collect()
                    } else {
                        // Find by node_id or name
                        let node_id = if let Some(_p) = registry.peers.get(&peer) {
                            peer.clone()
                        } else if let Some(p) = registry.peers.values().find(|p| p.name == peer) {
                            p.node_id.clone()
                        } else {
                            // Treat as raw node_id not in registry
                            peer.clone()
                        };
                        let name = registry
                            .peers
                            .get(&node_id)
                            .map(|p| p.name.clone())
                            .unwrap_or_else(|| node_id[..12.min(node_id.len())].to_string());
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
                                    let caps = crate::modes::rpc::peers::PeerCapabilities {
                                        accepts_prompts: result
                                            .get("accepts_prompts")
                                            .and_then(|v| v.as_bool())
                                            .unwrap_or(false),
                                        agents: result
                                            .get("agents")
                                            .and_then(|v| v.as_array())
                                            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                            .unwrap_or_default(),
                                        tools: result
                                            .get("tools")
                                            .and_then(|v| v.as_array())
                                            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                            .unwrap_or_default(),
                                        tags: result
                                            .get("tags")
                                            .and_then(|v| v.as_array())
                                            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                            .unwrap_or_default(),
                                        version: result.get("version").and_then(|v| v.as_str()).map(String::from),
                                    };
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
                    registry.save(&registry_path).map_err(|e| crate::error::Error::Io { source: e })?;
                    Ok(())
                }
            }
        }
        RpcAction::Allow { node_id } => {
            let _: iroh::PublicKey = node_id.parse().map_err(|e| crate::error::Error::Config {
                message: format!("Invalid node ID: {}", e),
            })?;
            let acl_path = crate::modes::rpc::iroh::allowlist_path(&ctx.paths);
            let mut allowed = crate::modes::rpc::iroh::load_allowlist(&acl_path);
            allowed.insert(node_id.clone());
            crate::modes::rpc::iroh::save_allowlist(&acl_path, &allowed)
                .map_err(|e| crate::error::Error::Io { source: e })?;
            println!("Allowed peer: {}", &node_id[..12.min(node_id.len())]);
            println!("Total allowed: {}", allowed.len());
            Ok(())
        }
        RpcAction::Deny { node_id } => {
            let acl_path = crate::modes::rpc::iroh::allowlist_path(&ctx.paths);
            let mut allowed = crate::modes::rpc::iroh::load_allowlist(&acl_path);
            if allowed.remove(&node_id) {
                crate::modes::rpc::iroh::save_allowlist(&acl_path, &allowed)
                    .map_err(|e| crate::error::Error::Io { source: e })?;
                println!("Denied peer: {}", &node_id[..12.min(node_id.len())]);
                Ok(())
            } else {
                Err(crate::error::Error::Config {
                    message: "Peer not in allowlist".to_string(),
                })
            }
        }
        RpcAction::Allowed => {
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
        RpcAction::Discover { mdns, scan_secs } => {
            let registry_path = crate::modes::rpc::peers::registry_path(&ctx.paths);
            let mut registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);

            let endpoint = crate::modes::rpc::iroh::start_endpoint(&identity).await?;

            // mDNS LAN scan — discover new peers automatically
            if mdns {
                let duration = std::time::Duration::from_secs(scan_secs);
                let discovered = crate::modes::rpc::iroh::discover_mdns_peers(&endpoint, duration).await;

                if discovered.is_empty() {
                    println!("No new peers found via mDNS.");
                } else {
                    println!("mDNS discovered {} peer(s):", discovered.len());
                    for (eid, _addr) in &discovered {
                        let nid = eid.to_string();
                        let short = &nid[..12.min(nid.len())];
                        if !registry.peers.contains_key(&nid) {
                            registry.add(&nid, &format!("mdns-{}", short));
                            println!("  + {} (auto-added as mdns-{})", short, short);
                        } else {
                            println!("  = {} (already known)", short);
                        }
                    }
                    registry.save(&registry_path).map_err(|e| crate::error::Error::Io { source: e })?;
                    println!();
                }
            }

            let peers = registry.list().iter().map(|p| (p.node_id.clone(), p.name.clone())).collect::<Vec<_>>();

            if peers.is_empty() {
                println!("No known peers. Add some with: clankers rpc peers add <node-id> <name>");
                println!("Or use --mdns to scan the local network.");
                return Ok(());
            }

            println!("Probing {} peer(s)...\n", peers.len());

            let mut online = 0;
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
                        online += 1;
                        if let Some(result) = response.ok {
                            let prompts = result.get("accepts_prompts").and_then(|v| v.as_bool()).unwrap_or(false);
                            let tags: Vec<String> = result
                                .get("tags")
                                .and_then(|v| v.as_array())
                                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                .unwrap_or_default();
                            let prompt_icon = if prompts { "✓" } else { "✗" };
                            let tags_str = if tags.is_empty() {
                                String::new()
                            } else {
                                format!(" [{}]", tags.join(", "))
                            };
                            println!(
                                "  ● {} ({}) — {} prompts{}",
                                name,
                                &node_id[..12.min(node_id.len())],
                                prompt_icon,
                                tags_str
                            );

                            // Update registry
                            let caps = crate::modes::rpc::peers::PeerCapabilities {
                                accepts_prompts: prompts,
                                agents: result
                                    .get("agents")
                                    .and_then(|v| v.as_array())
                                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                    .unwrap_or_default(),
                                tools: result
                                    .get("tools")
                                    .and_then(|v| v.as_array())
                                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                    .unwrap_or_default(),
                                tags,
                                version: result.get("version").and_then(|v| v.as_str()).map(String::from),
                            };
                            registry.update_capabilities(node_id, caps);
                        } else {
                            println!("  ● {} — online", name);
                            registry.touch(node_id);
                        }
                    }
                    Ok(Err(_)) | Err(_) => {
                        println!("  ○ {} ({}) — offline", name, &node_id[..12.min(node_id.len())]);
                    }
                }
            }
            registry.save(&registry_path).map_err(|e| crate::error::Error::Io { source: e })?;
            println!("\n{}/{} peers online", online, peers.len());
            Ok(())
        }
        RpcAction::SendFile { node_id, file } => {
            let remote: iroh::PublicKey = node_id.parse().map_err(|e| crate::error::Error::Config {
                message: format!("Invalid node ID: {}", e),
            })?;
            let file_path = std::path::Path::new(&file);
            if !file_path.exists() {
                return Err(crate::error::Error::Config {
                    message: format!("File not found: {}", file),
                });
            }
            let endpoint = crate::modes::rpc::iroh::start_endpoint(&identity).await?;
            let file_size = std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
            println!("Sending '{}' ({} bytes) to {}...", file_path.display(), file_size, remote.fmt_short());
            let response = crate::modes::rpc::iroh::send_file(&endpoint, remote, file_path).await?;
            if let Some(result) = response.ok {
                let remote_path = result.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                let size = result.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
                println!("✓ Sent {} bytes → {}", size, remote_path);
                Ok(())
            } else if let Some(err) = response.error {
                Err(crate::error::Error::Provider {
                    message: format!("RPC error: {}", err),
                })
            } else {
                Err(crate::error::Error::Provider {
                    message: "No response from peer".to_string(),
                })
            }
        }
        RpcAction::RecvFile {
            node_id,
            remote_path,
            output,
        } => {
            let remote: iroh::PublicKey = node_id.parse().map_err(|e| crate::error::Error::Config {
                message: format!("Invalid node ID: {}", e),
            })?;
            let local_path = output.map(std::path::PathBuf::from).unwrap_or_else(|| {
                let name =
                    std::path::Path::new(&remote_path).file_name().and_then(|n| n.to_str()).unwrap_or("downloaded");
                std::path::PathBuf::from(name)
            });
            let endpoint = crate::modes::rpc::iroh::start_endpoint(&identity).await?;
            println!("Downloading '{}' from {} → {}...", remote_path, remote.fmt_short(), local_path.display());
            let total = crate::modes::rpc::iroh::recv_file(&endpoint, remote, &remote_path, &local_path).await?;
            println!("✓ Received {} bytes → {}", total, local_path.display());
            Ok(())
        }
    }
}
