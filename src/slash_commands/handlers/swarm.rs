//! Swarm slash command handlers.

use tokio_util::sync::CancellationToken;

use super::SlashContext;
use super::SlashHandler;
// Helper functions for safe panel access
use crate::tui::components::peers_panel::PeersPanel;
use crate::tui::components::subagent_panel::SubagentPanel;
use crate::tui::panel::PanelId;

/// Get a mutable reference to the peers panel, panicking if not found.
/// Centralizes the expect call to make it easier to audit and replace.
fn peers_panel_mut<'a>(ctx: &'a mut SlashContext<'_>) -> &'a mut PeersPanel {
    ctx.app.panels.downcast_mut::<PeersPanel>(PanelId::Peers).expect("peers panel")
}

/// Get a mutable reference to the subagent panel, panicking if not found.
/// Centralizes the expect call to make it easier to audit and replace.
fn subagent_panel_mut<'a>(ctx: &'a mut SlashContext<'_>) -> &'a mut SubagentPanel {
    ctx.app.panels.downcast_mut::<SubagentPanel>(PanelId::Subagents).expect("subagent panel")
}

/// Get an immutable reference to the subagent panel, panicking if not found.
/// Centralizes the expect call to make it easier to audit and replace.
fn subagent_panel_ref<'a>(ctx: &'a SlashContext<'_>) -> &'a SubagentPanel {
    ctx.app.panels.downcast_ref::<SubagentPanel>(PanelId::Subagents).expect("subagent panel")
}

pub struct WorkerHandler;

impl SlashHandler for WorkerHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "worker",
            description: "Spawn or list swarm workers",
            help: "Spawn a named worker in a Zellij pane, or list active workers.\n\n\
                   Usage:\n  \
                   /worker                   — list active workers\n  \
                   /worker <name> <task>      — spawn worker with a task\n  \
                   /worker <name>             — spawn an idle worker\n\n\
                   Requires running inside a Zellij session (clankers --zellij or clankers --swarm).",
            accepts_args: true,
            subcommands: vec![],
            leader_key: None,
        }
    }

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
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "share",
            description: "Share this Zellij session remotely",
            help: "Share the current Zellij session over the network via iroh.\n\n\
                   Usage:\n  \
                   /share              — share read-write\n  \
                   /share --read-only  — share read-only\n\n\
                   Requires running inside a Zellij session.",
            accepts_args: true,
            subcommands: vec![],
            leader_key: None,
        }
    }

    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        ctx.app.push_system("Share is not yet implemented without Zellij.".to_string(), true);
    }
}

pub struct SubagentsHandler;

impl SlashHandler for SubagentsHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "subagents",
            description: "List and manage subagents",
            help: "List running and completed subagents, or manage them.\n\n\
                   Usage:\n  \
                   /subagents             — list all subagents\n  \
                   /subagents kill <id>   — kill a running subagent\n  \
                   /subagents kill all    — kill all running subagents\n  \
                   /subagents remove <id> — remove a subagent entry from the panel\n  \
                   /subagents clear       — remove all completed/failed subagents",
            accepts_args: true,
            subcommands: vec![
                ("kill <id>", "kill a running subagent"),
                ("kill all", "kill all running subagents"),
                ("remove <id>", "remove a subagent entry"),
                ("clear", "remove all completed/failed subagents"),
            ],
            leader_key: None,
        }
    }

    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if args.is_empty() {
            // List all subagents
            let subagent_panel = subagent_panel_ref(ctx);
            ctx.app.push_system(subagent_panel.summary(), false);
        } else {
            let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
            let subcmd = parts[0].trim();
            let subcmd_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

            let subagent_panel = subagent_panel_mut(ctx);
            match subcmd {
                "kill" => {
                    if subcmd_args == "all" {
                        // Kill all running subagents
                        let running: Vec<String> = subagent_panel
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
                        let matched = subagent_panel
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
                        let matched = subagent_panel
                            .entries
                            .iter()
                            .find(|e| e.id == target || e.name == target || e.id.contains(&target))
                            .map(|e| e.id.clone());
                        if let Some(id) = matched {
                            subagent_panel.remove_by_id(&id);
                            ctx.app.push_system(format!("Removed subagent '{}'.", id), false);
                        } else {
                            ctx.app.push_system(format!("No subagent matching '{}'.", subcmd_args), true);
                        }
                    }
                }
                "clear" => {
                    subagent_panel.clear_done();
                    ctx.app.push_system("Cleared completed/failed subagents.".to_string(), false);
                }
                _ => {
                    ctx.app.push_system(format!("Unknown subcommand '{}'. Use: kill, remove, clear", subcmd), true);
                }
            }
        }
    }
}

// Helper functions for PeersHandler subcommands

fn handle_peers_status(ctx: &mut SlashContext<'_>) {
    let paths = crate::config::ClankersPaths::get();
    let registry_path = crate::modes::rpc::peers::registry_path(paths);
    let registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
    let entries = crate::tui::components::peers_panel::entries_from_registry(&registry, chrono::Duration::minutes(5));
    let count = entries.len();
    peers_panel_mut(ctx).set_peers(entries);
    ctx.app.push_system(format!("{} peer(s) in registry.", count), false);
}

fn handle_peers_add(subcmd_args: &str, ctx: &mut SlashContext<'_>) {
    let parts: Vec<&str> = subcmd_args.splitn(2, char::is_whitespace).collect();
    if parts.len() < 2 {
        ctx.app.push_system("Usage: /peers add <node-id> <name>".to_string(), true);
    } else {
        let node_id = parts[0].trim();
        let name = parts[1].trim();
        let paths = crate::config::ClankersPaths::get();
        let registry_path = crate::modes::rpc::peers::registry_path(paths);
        let mut registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
        registry.add(node_id, name);
        match registry.save(&registry_path) {
            Ok(()) => {
                ctx.app
                    .push_system(format!("Added peer '{}' ({}…)", name, &node_id[..12.min(node_id.len())]), false);
                let entries =
                    crate::tui::components::peers_panel::entries_from_registry(&registry, chrono::Duration::minutes(5));
                peers_panel_mut(ctx).set_peers(entries);
            }
            Err(e) => ctx.app.push_system(format!("Failed to save registry: {}", e), true),
        }
    }
}

fn handle_peers_remove(subcmd_args: &str, ctx: &mut SlashContext<'_>) {
    if subcmd_args.is_empty() {
        ctx.app.push_system("Usage: /peers remove <name-or-id>".to_string(), true);
    } else {
        let paths = crate::config::ClankersPaths::get();
        let registry_path = crate::modes::rpc::peers::registry_path(paths);
        let mut registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
        // Try as node_id first, then by name
        let removed = if registry.remove(subcmd_args) {
            true
        } else {
            let found = registry.peers.values().find(|p| p.name == subcmd_args).map(|p| p.node_id.clone());
            if let Some(nid) = found {
                registry.remove(&nid)
            } else {
                false
            }
        };
        if removed {
            let _ = registry.save(&registry_path);
            ctx.app.push_system(format!("Removed peer '{}'.", subcmd_args), false);
            let entries =
                crate::tui::components::peers_panel::entries_from_registry(&registry, chrono::Duration::minutes(5));
            peers_panel_mut(ctx).set_peers(entries);
        } else {
            ctx.app.push_system(format!("Peer '{}' not found.", subcmd_args), true);
        }
    }
}

fn handle_peers_probe(subcmd_args: &str, ctx: &mut SlashContext<'_>) {
    let paths = crate::config::ClankersPaths::get();
    let registry_path = crate::modes::rpc::peers::registry_path(paths);
    let identity_path = crate::modes::rpc::iroh::identity_path(paths);

    if subcmd_args.is_empty() || subcmd_args == "all" {
        // Probe all peers
        let registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
        let peer_ids: Vec<String> = registry.peers.keys().cloned().collect();
        if peer_ids.is_empty() {
            ctx.app.push_system("No peers to probe.".to_string(), false);
        } else {
            ctx.app.push_system(format!("Probing {} peer(s)...", peer_ids.len()), false);
            for nid in &peer_ids {
                peers_panel_mut(ctx).update_status(nid, crate::tui::components::peers_panel::PeerStatus::Probing);
            }
            let ptx = ctx.panel_tx.clone();
            let rp = registry_path.clone();
            let ip = identity_path.clone();
            for nid in peer_ids {
                let ptx = ptx.clone();
                let rp = rp.clone();
                let ip = ip.clone();
                tokio::spawn(async move {
                    crate::modes::peers_background::probe_peer_background(nid, rp, ip, ptx).await;
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
        peers_panel_mut(ctx).update_status(&node_id, crate::tui::components::peers_panel::PeerStatus::Probing);
        ctx.app.push_system(format!("Probing {}...", &node_id[..12.min(node_id.len())]), false);
        let ptx = ctx.panel_tx.clone();
        tokio::spawn(async move {
            crate::modes::peers_background::probe_peer_background(node_id, registry_path, identity_path, ptx).await;
        });
    }
}

fn handle_peers_discover(ctx: &mut SlashContext<'_>) {
    ctx.app.push_system("Scanning LAN via mDNS (5s)...".to_string(), false);
    let paths = crate::config::ClankersPaths::get();
    let registry_path = crate::modes::rpc::peers::registry_path(paths);
    let identity_path = crate::modes::rpc::iroh::identity_path(paths);
    let ptx = ctx.panel_tx.clone();
    tokio::spawn(async move {
        crate::modes::peers_background::discover_peers_background(registry_path, identity_path, ptx).await;
    });
}

fn handle_peers_allow(subcmd_args: &str, ctx: &mut SlashContext<'_>) {
    if subcmd_args.is_empty() {
        ctx.app.push_system("Usage: /peers allow <node-id>".to_string(), true);
    } else {
        let paths = crate::config::ClankersPaths::get();
        let acl_path = crate::modes::rpc::iroh::allowlist_path(paths);
        let mut allowed = crate::modes::rpc::iroh::load_allowlist(&acl_path);
        allowed.insert(subcmd_args.to_string());
        match crate::modes::rpc::iroh::save_allowlist(&acl_path, &allowed) {
            Ok(()) => {
                ctx.app.push_system(format!("Allowed peer {}…", &subcmd_args[..12.min(subcmd_args.len())]), false)
            }
            Err(e) => ctx.app.push_system(format!("Failed: {}", e), true),
        }
    }
}

fn handle_peers_deny(subcmd_args: &str, ctx: &mut SlashContext<'_>) {
    if subcmd_args.is_empty() {
        ctx.app.push_system("Usage: /peers deny <node-id>".to_string(), true);
    } else {
        let paths = crate::config::ClankersPaths::get();
        let acl_path = crate::modes::rpc::iroh::allowlist_path(paths);
        let mut allowed = crate::modes::rpc::iroh::load_allowlist(&acl_path);
        if allowed.remove(subcmd_args) {
            let _ = crate::modes::rpc::iroh::save_allowlist(&acl_path, &allowed);
            ctx.app.push_system(format!("Denied peer {}…", &subcmd_args[..12.min(subcmd_args.len())]), false);
        } else {
            ctx.app.push_system("Peer not in allowlist.".to_string(), true);
        }
    }
}

fn handle_peers_server(subcmd_args: &str, ctx: &mut SlashContext<'_>) {
    match subcmd_args {
        "on" | "start" => {
            ctx.app.push_system(
                "Use `clankers rpc start` to run the RPC server (embedded server coming soon).".to_string(),
                false,
            );
        }
        "off" | "stop" => {
            ctx.app.push_system("Server control not yet available in TUI.".to_string(), false);
        }
        _ => {
            if peers_panel_mut(ctx).server_running {
                ctx.app.push_system("Embedded RPC server: running".to_string(), false);
            } else {
                ctx.app.push_system("Embedded RPC server: not running".to_string(), false);
            }
        }
    }
}

pub struct PeersHandler;

impl SlashHandler for PeersHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "peers",
            description: "Manage swarm peers",
            help: "View and manage P2P swarm peers.\n\n\
                   Usage:\n  \
                   /peers                      — list all peers (switches to peers panel)\n  \
                   /peers add <node-id> <name>  — add a peer to the registry\n  \
                   /peers remove <name-or-id>   — remove a peer\n  \
                   /peers probe [name-or-id]    — probe a peer (or all peers)\n  \
                   /peers discover              — scan LAN via mDNS for new peers\n  \
                   /peers allow <node-id>       — add to allowlist\n  \
                   /peers deny <node-id>        — remove from allowlist\n  \
                   /peers server [on|off]       — start/stop embedded RPC server",
            accepts_args: true,
            subcommands: vec![
                ("add <node-id> <name>", "add a peer"),
                ("remove <name-or-id>", "remove a peer"),
                ("probe [name-or-id]", "probe a peer or all peers"),
                ("discover", "scan LAN via mDNS"),
                ("allow <node-id>", "add to allowlist"),
                ("deny <node-id>", "remove from allowlist"),
                ("server [on|off]", "start/stop RPC server"),
            ],
            leader_key: None,
        }
    }

    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        // Switch to peers panel tab
        ctx.app.focus_panel(PanelId::Peers);

        if args.is_empty() {
            handle_peers_status(ctx);
        } else {
            let (subcmd, subcmd_args) = args.split_once(char::is_whitespace).unwrap_or((args, ""));
            let subcmd_args = subcmd_args.trim();

            match subcmd {
                "add" => handle_peers_add(subcmd_args, ctx),
                "remove" | "rm" => handle_peers_remove(subcmd_args, ctx),
                "probe" => handle_peers_probe(subcmd_args, ctx),
                "discover" => handle_peers_discover(ctx),
                "allow" => handle_peers_allow(subcmd_args, ctx),
                "deny" => handle_peers_deny(subcmd_args, ctx),
                "server" => handle_peers_server(subcmd_args, ctx),
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
