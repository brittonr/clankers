//! Swarm slash command handlers.

use super::SlashContext;
use super::SlashHandler;
use tokio_util::sync::CancellationToken;

pub struct WorkerHandler;

impl SlashHandler for WorkerHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if args.is_empty() {
            ctx.app.push_system(
                "Usage: /worker <name> <task>\n\nSpawns a clankers subprocess as a named worker. Output appears in the subagent panel.".to_string(),
                false,
            );
        } else {
            let (worker_name, task) = match args.split_once(char::is_whitespace) {
                Some((name, rest)) => (name.trim().to_string(), rest.trim().to_string()),
                None => {
                    ctx.app.push_system("Usage: /worker <name> <task>".to_string(), true);
                    return;
                }
            };
            ctx.app.push_system(format!("Worker '{}' started. See subagent panel →", worker_name), false);
            let ptx = ctx.panel_tx.clone();
            tokio::spawn(async move {
                let signal = CancellationToken::new();
                let _ = crate::tools::delegate::run_worker_subprocess(
                    &worker_name,
                    &task,
                    None,
                    None,
                    Some(&ptx),
                    signal,
                    None,
                )
                .await;
            });
        }
    }
}

pub struct ShareHandler;

impl SlashHandler for ShareHandler {
    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        ctx.app.push_system("Share is not yet implemented without Zellij.".to_string(), true);
    }
}

pub struct SubagentsHandler;

impl SlashHandler for SubagentsHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if args.is_empty() {
            // List all subagents
            ctx.app.push_system(ctx.app.subagent_panel.summary(), false);
        } else {
            let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
            let subcmd = parts[0].trim();
            let subcmd_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

            match subcmd {
                "kill" => {
                    if subcmd_args == "all" {
                        // Kill all running subagents
                        let running: Vec<String> = ctx.app
                            .subagent_panel
                            .entries
                            .iter()
                            .filter(|e| e.status == crate::tui::components::subagent_panel::SubagentStatus::Running)
                            .map(|e| e.id.clone())
                            .collect();
                        if running.is_empty() {
                            ctx.app.push_system("No running subagents to kill.".to_string(), false);
                        } else {
                            for id in &running {
                                let _ = ctx.panel_tx.send(
                                    crate::tui::components::subagent_event::SubagentEvent::KillRequest {
                                        id: id.clone(),
                                    },
                                );
                            }
                            ctx.app.push_system(format!("Kill requested for {} subagent(s).", running.len()), false);
                        }
                    } else if subcmd_args.is_empty() {
                        ctx.app.push_system("Usage: /subagents kill <id> or /subagents kill all".to_string(), true);
                    } else {
                        let target = subcmd_args.to_string();
                        // Try partial match on id or name
                        let matched = ctx.app
                            .subagent_panel
                            .entries
                            .iter()
                            .find(|e| e.id == target || e.name == target || e.id.contains(&target))
                            .map(|e| e.id.clone());
                        if let Some(id) = matched {
                            let _ =
                                ctx.panel_tx.send(crate::tui::components::subagent_event::SubagentEvent::KillRequest {
                                    id: id.clone(),
                                });
                            ctx.app.push_system(format!("Kill requested for subagent '{}'.", id), false);
                        } else {
                            ctx.app.push_system(format!("No subagent matching '{}'.", subcmd_args), true);
                        }
                    }
                }
                "remove" | "rm" => {
                    if subcmd_args.is_empty() {
                        ctx.app.push_system("Usage: /subagents remove <id>".to_string(), true);
                    } else {
                        let target = subcmd_args.to_string();
                        let matched = ctx.app
                            .subagent_panel
                            .entries
                            .iter()
                            .find(|e| e.id == target || e.name == target || e.id.contains(&target))
                            .map(|e| e.id.clone());
                        if let Some(id) = matched {
                            ctx.app.subagent_panel.remove_by_id(&id);
                            ctx.app.push_system(format!("Removed subagent '{}'.", id), false);
                        } else {
                            ctx.app.push_system(format!("No subagent matching '{}'.", subcmd_args), true);
                        }
                    }
                }
                "clear" => {
                    ctx.app.subagent_panel.clear_done();
                    ctx.app.push_system("Cleared completed/failed subagents.".to_string(), false);
                }
                _ => {
                    ctx.app.push_system(format!("Unknown subcommand '{}'. Use: kill, remove, clear", subcmd), true);
                }
            }
        }
    }
}

pub struct PeersHandler;

impl SlashHandler for PeersHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        // Switch to peers panel tab
        ctx.app.panel_tab = crate::tui::app::PanelTab::Peers;
        ctx.app.right_panel_tab = crate::tui::app::PanelTab::Peers;
        ctx.app.panel_focused = true;

        if args.is_empty() {
            // Just show the panel — refresh peers from registry
            let paths = crate::config::ClankersPaths::resolve();
            let registry_path = crate::modes::rpc::peers::registry_path(&paths);
            let registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
            let entries =
                crate::tui::components::peers_panel::entries_from_registry(&registry, chrono::Duration::minutes(5));
            let count = entries.len();
            ctx.app.peers_panel.set_peers(entries);
            ctx.app.push_system(format!("{} peer(s) in registry.", count), false);
        } else {
            let (subcmd, subcmd_args) = args.split_once(char::is_whitespace).unwrap_or((args, ""));
            let subcmd_args = subcmd_args.trim();
            match subcmd {
                "add" => {
                    let parts: Vec<&str> = subcmd_args.splitn(2, char::is_whitespace).collect();
                    if parts.len() < 2 {
                        ctx.app.push_system("Usage: /peers add <node-id> <name>".to_string(), true);
                    } else {
                        let node_id = parts[0].trim();
                        let name = parts[1].trim();
                        let paths = crate::config::ClankersPaths::resolve();
                        let registry_path = crate::modes::rpc::peers::registry_path(&paths);
                        let mut registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
                        registry.add(node_id, name);
                        match registry.save(&registry_path) {
                            Ok(()) => {
                                ctx.app.push_system(
                                    format!("Added peer '{}' ({}…)", name, &node_id[..12.min(node_id.len())]),
                                    false,
                                );
                                let entries = crate::tui::components::peers_panel::entries_from_registry(
                                    &registry,
                                    chrono::Duration::minutes(5),
                                );
                                ctx.app.peers_panel.set_peers(entries);
                            }
                            Err(e) => ctx.app.push_system(format!("Failed to save registry: {}", e), true),
                        }
                    }
                }
                "remove" | "rm" => {
                    if subcmd_args.is_empty() {
                        ctx.app.push_system("Usage: /peers remove <name-or-id>".to_string(), true);
                    } else {
                        let paths = crate::config::ClankersPaths::resolve();
                        let registry_path = crate::modes::rpc::peers::registry_path(&paths);
                        let mut registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
                        // Try as node_id first, then by name
                        let removed = if registry.remove(subcmd_args) {
                            true
                        } else {
                            let found =
                                registry.peers.values().find(|p| p.name == subcmd_args).map(|p| p.node_id.clone());
                            if let Some(nid) = found {
                                registry.remove(&nid)
                            } else {
                                false
                            }
                        };
                        if removed {
                            let _ = registry.save(&registry_path);
                            ctx.app.push_system(format!("Removed peer '{}'.", subcmd_args), false);
                            let entries = crate::tui::components::peers_panel::entries_from_registry(
                                &registry,
                                chrono::Duration::minutes(5),
                            );
                            ctx.app.peers_panel.set_peers(entries);
                        } else {
                            ctx.app.push_system(format!("Peer '{}' not found.", subcmd_args), true);
                        }
                    }
                }
                "probe" => {
                    let paths = crate::config::ClankersPaths::resolve();
                    let registry_path = crate::modes::rpc::peers::registry_path(&paths);
                    let identity_path = crate::modes::rpc::iroh::identity_path(&paths);

                    if subcmd_args.is_empty() || subcmd_args == "all" {
                        // Probe all peers
                        let registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
                        let peer_ids: Vec<String> = registry.peers.keys().cloned().collect();
                        if peer_ids.is_empty() {
                            ctx.app.push_system("No peers to probe.".to_string(), false);
                        } else {
                            ctx.app.push_system(format!("Probing {} peer(s)...", peer_ids.len()), false);
                            for nid in &peer_ids {
                                ctx.app.peers_panel
                                    .update_status(nid, crate::tui::components::peers_panel::PeerStatus::Probing);
                            }
                            let ptx = ctx.panel_tx.clone();
                            let rp = registry_path.clone();
                            let ip = identity_path.clone();
                            for nid in peer_ids {
                                let ptx = ptx.clone();
                                let rp = rp.clone();
                                let ip = ip.clone();
                                tokio::spawn(async move {
                                    crate::modes::interactive::probe_peer_background(nid, rp, ip, ptx).await;
                                });
                            }
                        }
                    } else {
                        // Probe specific peer
                        let registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
                        let node_id = registry
                            .peers
                            .values()
                            .find(|p| p.name == subcmd_args)
                            .map(|p| p.node_id.clone())
                            .unwrap_or_else(|| subcmd_args.to_string());
                        ctx.app.peers_panel
                            .update_status(&node_id, crate::tui::components::peers_panel::PeerStatus::Probing);
                        ctx.app.push_system(format!("Probing {}...", &node_id[..12.min(node_id.len())]), false);
                        let ptx = ctx.panel_tx.clone();
                        tokio::spawn(async move {
                            crate::modes::interactive::probe_peer_background(node_id, registry_path, identity_path, ptx).await;
                        });
                    }
                }
                "discover" => {
                    ctx.app.push_system("Scanning LAN via mDNS (5s)...".to_string(), false);
                    let paths = crate::config::ClankersPaths::resolve();
                    let registry_path = crate::modes::rpc::peers::registry_path(&paths);
                    let identity_path = crate::modes::rpc::iroh::identity_path(&paths);
                    let ptx = ctx.panel_tx.clone();
                    tokio::spawn(async move {
                        crate::modes::interactive::discover_peers_background(registry_path, identity_path, ptx).await;
                    });
                }
                "allow" => {
                    if subcmd_args.is_empty() {
                        ctx.app.push_system("Usage: /peers allow <node-id>".to_string(), true);
                    } else {
                        let paths = crate::config::ClankersPaths::resolve();
                        let acl_path = crate::modes::rpc::iroh::allowlist_path(&paths);
                        let mut allowed = crate::modes::rpc::iroh::load_allowlist(&acl_path);
                        allowed.insert(subcmd_args.to_string());
                        match crate::modes::rpc::iroh::save_allowlist(&acl_path, &allowed) {
                            Ok(()) => ctx.app.push_system(
                                format!("Allowed peer {}…", &subcmd_args[..12.min(subcmd_args.len())]),
                                false,
                            ),
                            Err(e) => ctx.app.push_system(format!("Failed: {}", e), true),
                        }
                    }
                }
                "deny" => {
                    if subcmd_args.is_empty() {
                        ctx.app.push_system("Usage: /peers deny <node-id>".to_string(), true);
                    } else {
                        let paths = crate::config::ClankersPaths::resolve();
                        let acl_path = crate::modes::rpc::iroh::allowlist_path(&paths);
                        let mut allowed = crate::modes::rpc::iroh::load_allowlist(&acl_path);
                        if allowed.remove(subcmd_args) {
                            let _ = crate::modes::rpc::iroh::save_allowlist(&acl_path, &allowed);
                            ctx.app.push_system(
                                format!("Denied peer {}…", &subcmd_args[..12.min(subcmd_args.len())]),
                                false,
                            );
                        } else {
                            ctx.app.push_system("Peer not in allowlist.".to_string(), true);
                        }
                    }
                }
                "server" => match subcmd_args {
                    "on" | "start" => {
                        ctx.app.push_system(
                            "Use `clankers rpc start` to run the RPC server (embedded server coming soon)."
                                .to_string(),
                            false,
                        );
                    }
                    "off" | "stop" => {
                        ctx.app.push_system("Server control not yet available in TUI.".to_string(), false);
                    }
                    _ => {
                        if ctx.app.peers_panel.server_running {
                            ctx.app.push_system("Embedded RPC server: running".to_string(), false);
                        } else {
                            ctx.app.push_system("Embedded RPC server: not running".to_string(), false);
                        }
                    }
                },
                _ => {
                    ctx.app.push_system(
                        format!(
                            "Unknown subcommand '{}'. Available: add, remove, probe, discover, allow, deny, server",
                            subcmd
                        ),
                        true,
                    );
                }
            }
        }
    }
}
