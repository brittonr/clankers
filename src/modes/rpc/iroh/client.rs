//! Client-side RPC and file transfer

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use iroh::Endpoint;
use iroh::EndpointAddr;
use iroh::PublicKey;
use serde_json::json;
use tracing::debug;
use tracing::info;
use tracing::warn;

use super::ALPN;
use super::protocol::read_frame;
use super::protocol::write_frame;
use crate::modes::rpc::protocol::Request;
use crate::modes::rpc::protocol::Response;

// ── Client: send RPC ────────────────────────────────────────────────────────

/// Send an RPC request and return the single final response.
/// For streaming, use `send_rpc_streaming`.
pub async fn send_rpc(
    endpoint: &Endpoint,
    remote: impl Into<EndpointAddr>,
    request: &Request,
) -> Result<Response, crate::error::Error> {
    let (_, response) = send_rpc_streaming(endpoint, remote, request, |_| {}).await?;
    Ok(response)
}

/// Send an RPC request. Calls `on_notification` for each intermediate
/// notification frame, then returns the final response.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(unbounded_loop, reason = "event loop; bounded by stream end")
)]
pub async fn send_rpc_streaming(
    endpoint: &Endpoint,
    remote: impl Into<EndpointAddr>,
    request: &Request,
    mut on_notification: impl FnMut(&serde_json::Value),
) -> Result<(Vec<serde_json::Value>, Response), crate::error::Error> {
    let addr: EndpointAddr = remote.into();
    let conn = endpoint.connect(addr, ALPN).await.map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to connect to peer: {}", e),
    })?;

    let (mut send, mut recv) = conn.open_bi().await.map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to open stream: {}", e),
    })?;

    // Send request
    write_frame(&mut send, &serde_json::to_vec(request).unwrap_or_default()).await?;
    send.finish().map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to finish send: {}", e),
    })?;

    // Read frames until we get a final response (has "ok" or "error" field).
    // Intermediate frames without those fields are streaming notifications.
    let mut notifications = Vec::new();
    loop {
        let data = read_frame(&mut recv).await?;
        let value: serde_json::Value = serde_json::from_slice(&data).map_err(|e| crate::error::Error::Provider {
            message: format!("Failed to parse frame: {}", e),
        })?;

        if value.get("ok").is_some() || value.get("error").is_some() {
            let response: Response = serde_json::from_value(value).map_err(|e| crate::error::Error::Provider {
                message: format!("Failed to parse response: {}", e),
            })?;
            return Ok((notifications, response));
        }
        on_notification(&value);
        notifications.push(value);
    }
}

// ── Client: file transfer ───────────────────────────────────────────────────

/// Send a file to a remote peer.
///
/// Opens a bidirectional stream, sends a `file.send` request with metadata,
/// then streams the raw file bytes. The remote peer saves the file and
/// responds with the path where it was stored.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(unbounded_loop, reason = "event loop; bounded by stream end")
)]
pub async fn send_file(
    endpoint: &Endpoint,
    remote: impl Into<EndpointAddr>,
    file_path: &Path,
) -> Result<Response, crate::error::Error> {
    let metadata = std::fs::metadata(file_path).map_err(|e| crate::error::Error::Provider {
        message: format!("Cannot stat file: {}", e),
    })?;
    let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("unnamed").to_string();
    let file_size = metadata.len();

    let addr: EndpointAddr = remote.into();
    let conn = endpoint.connect(addr, ALPN).await.map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to connect to peer: {}", e),
    })?;

    let (mut send, mut recv) = conn.open_bi().await.map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to open stream: {}", e),
    })?;

    // Send the file.send request
    let request = Request::new("file.send", json!({ "name": file_name, "size": file_size }));
    write_frame(&mut send, &serde_json::to_vec(&request).unwrap_or_default()).await?;

    // Stream the file data in chunks
    let mut file = tokio::fs::File::open(file_path).await.map_err(|e| crate::error::Error::Provider {
        message: format!("Cannot open file: {}", e),
    })?;
    let mut buf = vec![0u8; 64 * 1024]; // 64KB chunks
    loop {
        let n =
            tokio::io::AsyncReadExt::read(&mut file, &mut buf)
                .await
                .map_err(|e| crate::error::Error::Provider {
                    message: format!("File read error: {}", e),
                })?;
        if n == 0 {
            break;
        }
        send.write_all(&buf[..n]).await.map_err(io_err)?;
    }
    send.finish().map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to finish send: {}", e),
    })?;

    // Read the response
    let data = read_frame(&mut recv).await?;
    let response: Response = serde_json::from_slice(&data).map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to parse response: {}", e),
    })?;
    Ok(response)
}

/// Request a file from a remote peer.
///
/// Sends a `file.recv` request. The server responds with a header frame
/// containing the file size, followed by the raw file bytes.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(unbounded_loop, reason = "event loop; bounded by stream end")
)]
pub async fn recv_file(
    endpoint: &Endpoint,
    remote: impl Into<EndpointAddr>,
    remote_path: &str,
    local_path: &Path,
) -> Result<u64, crate::error::Error> {
    let addr: EndpointAddr = remote.into();
    let conn = endpoint.connect(addr, ALPN).await.map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to connect to peer: {}", e),
    })?;

    let (mut send, mut recv) = conn.open_bi().await.map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to open stream: {}", e),
    })?;

    // Send the file.recv request
    let request = Request::new("file.recv", json!({ "path": remote_path }));
    write_frame(&mut send, &serde_json::to_vec(&request).unwrap_or_default()).await?;
    send.finish().map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to finish send: {}", e),
    })?;

    // Read the header response
    let data = read_frame(&mut recv).await?;
    let response: Response = serde_json::from_slice(&data).map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to parse response: {}", e),
    })?;

    if let Some(err) = response.error {
        return Err(crate::error::Error::Provider {
            message: format!("Remote error: {}", err),
        });
    }

    let file_size = response.ok.as_ref().and_then(|r| r.get("size")).and_then(|v| v.as_u64()).unwrap_or(0);

    // Read the raw file data
    if let Some(parent) = local_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| crate::error::Error::Provider {
            message: format!("Cannot create directory: {}", e),
        })?;
    }

    let mut file = tokio::fs::File::create(local_path).await.map_err(|e| crate::error::Error::Provider {
        message: format!("Cannot create file: {}", e),
    })?;

    let mut total = 0u64;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match recv.read(&mut buf).await {
            Ok(Some(n)) => n,
            Ok(None) => break,
            Err(e) => {
                return Err(crate::error::Error::Provider {
                    message: format!("Stream read error: {}", e),
                });
            }
        };
        tokio::io::AsyncWriteExt::write_all(&mut file, &buf[..n])
            .await
            .map_err(|e| crate::error::Error::Provider {
                message: format!("File write error: {}", e),
            })?;
        total += n as u64;
    }

    // Flush to ensure all data is written to disk before returning.
    // Without this, the tokio::fs::File drop may not complete the write
    // synchronously, causing callers that immediately read the file to
    // see stale/empty content.
    tokio::io::AsyncWriteExt::shutdown(&mut file).await.map_err(|e| crate::error::Error::Provider {
        message: format!("File flush error: {}", e),
    })?;

    info!("Received file {} ({} bytes)", local_path.display(), total);
    if file_size > 0 && total != file_size {
        warn!("File size mismatch: expected {} bytes, got {} bytes", file_size, total);
    }

    Ok(total)
}

// ── Peer health / heartbeat ─────────────────────────────────────────────────

/// Periodically probe all known peers and update the registry.
///
/// Runs in a background task. Probes each peer with a "status" RPC every
/// `interval`. Updates capabilities and last_seen on success, or marks
/// peers as stale after repeated failures.
pub async fn run_heartbeat(
    endpoint: Arc<Endpoint>,
    registry_path: PathBuf,
    interval: std::time::Duration,
    cancel: tokio_util::sync::CancellationToken,
) {
    info!("Heartbeat started (interval: {:?})", interval);
    loop {
        tokio::select! {
            () = tokio::time::sleep(interval) => {}
            () = cancel.cancelled() => {
                info!("Heartbeat stopped");
                return;
            }
        }

        let mut registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
        let peer_ids: Vec<String> = registry.peers.keys().cloned().collect();

        if peer_ids.is_empty() {
            continue;
        }

        debug!("Heartbeat: probing {} peer(s)", peer_ids.len());

        for node_id in &peer_ids {
            let remote: PublicKey = match node_id.parse() {
                Ok(pk) => pk,
                Err(_) => continue,
            };

            let request = Request::new("status", json!({}));
            match tokio::time::timeout(std::time::Duration::from_secs(10), send_rpc(&endpoint, remote, &request)).await
            {
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
                        registry.update_capabilities(node_id, caps);
                        debug!("Heartbeat: {} online", &node_id[..12.min(node_id.len())]);
                    } else {
                        registry.touch(node_id);
                    }
                }
                _ => {
                    debug!("Heartbeat: {} unreachable", &node_id[..12.min(node_id.len())]);
                    // Don't remove — just leave last_seen stale
                }
            }
        }

        if let Err(e) = registry.save(&registry_path) {
            warn!("Heartbeat: failed to save registry: {}", e);
        }
    }
}

// ── mDNS LAN discovery ─────────────────────────────────────────────────────

/// Scan the local network for clankers peers via mDNS.
///
/// Creates a dedicated mDNS listener, subscribes to discovery events for
/// `duration`, and returns discovered peer endpoint IDs. These can then be
/// probed with "status" to get capabilities before adding to the registry.
pub async fn discover_mdns_peers(
    endpoint: &Endpoint,
    duration: std::time::Duration,
) -> Vec<(iroh::EndpointId, Option<iroh::address_lookup::EndpointInfo>)> {
    use futures::StreamExt;
    use iroh::address_lookup::mdns::DiscoveryEvent;
    use iroh::address_lookup::mdns::MdnsAddressLookup;

    use super::MDNS_SERVICE_NAME;

    info!("Scanning LAN for clankers peers via mDNS ({:?})...", duration);

    // Build a dedicated mDNS instance for subscribing to events.
    // The builder needs the endpoint ID to filter out self-announcements.
    let mdns = match MdnsAddressLookup::builder().service_name(MDNS_SERVICE_NAME).build(endpoint.id()) {
        Ok(m) => m,
        Err(e) => {
            warn!("Failed to create mDNS scanner: {}", e);
            return Vec::new();
        }
    };

    let mut stream = mdns.subscribe().await;
    let mut discovered = Vec::new();

    let deadline = tokio::time::sleep(duration);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            () = &mut deadline => break,
            event = stream.next() => {
                match event {
                    Some(DiscoveryEvent::Discovered { endpoint_info, .. }) => {
                        let eid = endpoint_info.endpoint_id;
                        // Skip ourselves
                        if eid == endpoint.id() {
                            continue;
                        }
                        info!("mDNS: discovered peer {}", eid.fmt_short());
                        if !discovered.iter().any(|(id, _): &(iroh::EndpointId, _)| *id == eid) {
                            discovered.push((eid, Some(endpoint_info)));
                        }
                    }
                    Some(DiscoveryEvent::Expired { .. }) => {} // peer went offline
                    None => break,
                }
            }
        }
    }

    info!("mDNS scan complete: {} peer(s) found", discovered.len());
    discovered
}

fn io_err(e: impl std::fmt::Display) -> crate::error::Error {
    crate::error::Error::Provider {
        message: format!("IO error: {}", e),
    }
}
