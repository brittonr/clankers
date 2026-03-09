//! Background peer probe and discovery tasks.

/// Probe a single peer in the background. Updates the registry and sends
/// a status event back to the TUI via the panel channel.
pub(crate) async fn probe_peer_background(
    node_id: String,
    registry_path: std::path::PathBuf,
    identity_path: std::path::PathBuf,
    _panel_tx: tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
) {
    use crate::modes::rpc::iroh;
    use crate::modes::rpc::protocol::Request;

    let remote: ::iroh::PublicKey = match node_id.parse() {
        Ok(pk) => pk,
        Err(e) => {
            tracing::warn!("Invalid node ID '{}': {}", node_id, e);
            return;
        }
    };

    let identity = iroh::Identity::load_or_generate(&identity_path);
    let endpoint = match iroh::start_endpoint_no_mdns(&identity).await {
        Ok(ep) => ep,
        Err(e) => {
            tracing::warn!("Failed to start endpoint for probe: {}", e);
            return;
        }
    };
    let request = Request::new("status", serde_json::json!({}));
    let result =
        tokio::time::timeout(std::time::Duration::from_secs(10), iroh::send_rpc(&endpoint, remote, &request)).await;

    let mut registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);

    match result {
        Ok(Ok(response)) => {
            if let Some(result) = response.ok {
                let caps = crate::modes::rpc::peers::PeerCapabilities {
                    accepts_prompts: result.get("accepts_prompts").and_then(|v| v.as_bool()).unwrap_or(false),
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
                registry.update_capabilities(&node_id, caps);
                tracing::info!("Probed peer {}: online", &node_id[..12.min(node_id.len())]);
            } else {
                registry.touch(&node_id);
            }
        }
        _ => {
            tracing::info!("Probed peer {}: unreachable", &node_id[..12.min(node_id.len())]);
        }
    }

    let _ = registry.save(&registry_path);
}

/// Discover peers via mDNS in the background. Adds discovered peers to the
/// registry and probes them for capabilities.
pub(crate) async fn discover_peers_background(
    registry_path: std::path::PathBuf,
    identity_path: std::path::PathBuf,
    _panel_tx: tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
) {
    use crate::modes::rpc::iroh;

    let identity = iroh::Identity::load_or_generate(&identity_path);
    let endpoint = match iroh::start_endpoint(&identity).await {
        Ok(ep) => ep,
        Err(e) => {
            tracing::warn!("Failed to start endpoint for discovery: {}", e);
            return;
        }
    };

    let discovered = iroh::discover_mdns_peers(&endpoint, std::time::Duration::from_secs(5)).await;

    if discovered.is_empty() {
        tracing::info!("mDNS discovery: no peers found");
        return;
    }

    let mut registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);

    for (eid, _info) in &discovered {
        let node_id = eid.to_string();
        if !registry.peers.contains_key(&node_id) {
            let short = &node_id[..12.min(node_id.len())];
            registry.add(&node_id, &format!("mdns-{}", short));
            tracing::info!("Discovered new peer via mDNS: {}", short);
        }
    }

    let _ = registry.save(&registry_path);

    // Probe each discovered peer for capabilities
    for (eid, _info) in discovered {
        let node_id = eid.to_string();
        let rp = registry_path.clone();
        let ip = identity_path.clone();
        probe_peer_background(node_id, rp, ip, _panel_tx.clone()).await;
    }
}
