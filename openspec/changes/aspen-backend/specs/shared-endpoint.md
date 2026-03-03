# Shared Iroh Endpoint

## Summary

When running in cluster mode, clankers shares aspen's iroh QUIC endpoint
instead of creating its own.  Clankers registers its protocol handlers
(`clankers/rpc/1`, `clankers/chat/1`) on aspen's endpoint via ALPN
multiplexing.  This eliminates duplicate relay connections, duplicate
mDNS announcements, and port conflicts.

## Current State

Today clankers creates its own iroh endpoint in the daemon:

```rust
// src/modes/daemon.rs — current
let endpoint = ::iroh::Endpoint::builder()
    .secret_key(identity.secret_key.clone())
    .alpns(vec![iroh::ALPN.to_vec(), ALPN_CHAT.to_vec()])
    .address_lookup(mdns_service)
    .bind()
    .await?;
```

And aspen creates its own in the bootstrap sequence:

```rust
// aspen — current
let endpoint = iroh::Endpoint::builder()
    .secret_key(node_key)
    .alpns(vec![...many ALPNs...])
    .discovery(discovery_config)
    .bind()
    .await?;
```

Two endpoints = two QUIC ports, two relay connections, two mDNS services.

## Target State

In cluster mode, clankers gets its endpoint handle from aspen's
`NodeHandle::network.iroh_manager`:

```rust
// clankers-aspen bridge — target
pub struct SharedEndpoint {
    endpoint: iroh::Endpoint,
    /// Cancellation token for clankers protocol handlers
    cancel: CancellationToken,
}

impl SharedEndpoint {
    /// Register clankers protocols on aspen's endpoint.
    pub fn register(node_handle: &NodeHandle) -> Result<Self> {
        let endpoint = node_handle.network.iroh_manager.endpoint().clone();

        // Register clankers ALPNs
        endpoint.add_alpn(ALPN_RPC.to_vec())?;
        endpoint.add_alpn(ALPN_CHAT.to_vec())?;

        Ok(Self { endpoint, cancel: CancellationToken::new() })
    }

    /// Run the clankers accept loop on the shared endpoint.
    pub async fn run(&self, handler: ClankerProtocolHandler) -> Result<()> {
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => break,
                conn = self.endpoint.accept() => {
                    let Some(conn) = conn else { break };
                    let alpn = conn.alpn().await?;
                    match alpn.as_slice() {
                        ALPN_RPC => handler.handle_rpc(conn).await,
                        ALPN_CHAT => handler.handle_chat(conn).await,
                        _ => continue, // Not ours, aspen handles it
                    }
                }
            }
        }
        Ok(())
    }
}
```

## Identity

In cluster mode, the clankers node uses the **same iroh identity** as the
aspen node.  The `NodeId` (Ed25519 public key) is shared across both
systems.  This means:

- Peers see one identity, not two
- UCAN tokens reference one key
- Gossip announcements are unified

In standalone mode, clankers continues to generate/load its own identity
from `~/.clankers/identity.json`.

## Discovery

Aspen's discovery stack (mDNS, gossip, DHT, Pkarr) is reused.  Clankers
adds metadata to gossip announcements:

```rust
GossipAnnouncement {
    node_id: ...,
    capabilities: vec![
        "aspen-raft",       // standard aspen
        "clankers-daemon",  // clankers-specific
    ],
    metadata: {
        "clankers_model": "claude-sonnet-4-5",
        "clankers_version": "0.1.0",
        "clankers_tools": ["read", "write", "bash", "edit", ...],
    },
}
```

Other clankers nodes discover each other via gossip and can route
subagent jobs to nodes with specific tool/model capabilities.

## Standalone Fallback

When not connected to an aspen cluster, the current behavior is preserved:

```rust
enum EndpointMode {
    /// Own endpoint, own identity (current default)
    Standalone {
        endpoint: iroh::Endpoint,
        identity: Identity,
    },
    /// Shared with aspen, registered ALPNs
    Cluster {
        shared: SharedEndpoint,
    },
}
```

## Connection Reuse

The clankers-router (LLM proxy) also benefits.  Its iroh P2P tunnel
feature currently creates a third endpoint.  In cluster mode, the router
registers its ALPN (`clankers-router/proxy/1`) on the shared endpoint too:

```
One iroh endpoint serves:
  raft-auth               → aspen consensus
  aspen-client            → aspen client RPC
  iroh-blobs/0            → blob transfer
  iroh-gossip/0           → peer discovery
  clankers/rpc/1          → clankers JSON-RPC
  clankers/chat/1         → clankers chat sessions
  clankers-router/proxy/1 → OpenAI-compatible LLM proxy
```
