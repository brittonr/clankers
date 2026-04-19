//! Iroh tunnel — expose the HTTP proxy over iroh p2p connections.
//!
//! Implements iroh's `ProtocolHandler` to accept iroh QUIC connections and
//! tunnel them to the local axum HTTP server. Any iroh peer with the router's
//! node ID can access the OpenAI-compatible API without TCP connectivity.
//!
//! # Protocol
//!
//! Each iroh bidirectional stream is treated as a raw TCP tunnel to the local
//! HTTP server. The peer sends HTTP/1.1 requests and receives HTTP responses,
//! exactly as if connected via TCP. No additional framing or handshake.
//!
//! ALPN: `clanker-router-http/1`
//!
//! # Architecture
//!
//! This is inspired by [iroh-proxy-utils](https://github.com/n0-computer/iroh-proxy-utils)
//! but stripped down to the essentials: we only need the upstream (server) side,
//! and we don't need HTTP parsing or CONNECT handshakes since both endpoints
//! are under our control.
//!
//! ```text
//! Remote client
//!   │ iroh QUIC (bidirectional stream)
//!   ▼
//! IrohTunnel (ProtocolHandler)
//!   │ TCP connect to 127.0.0.1:4000
//!   ▼
//! axum server (OpenAI-compatible proxy)
//!   │ Router.complete()
//!   ▼
//! LLM providers (Anthropic, OpenAI, etc.)
//! ```

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use iroh::endpoint::Connection;
use iroh::protocol::AcceptError;
use iroh::protocol::ProtocolHandler;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tracing::debug;
use tracing::info;
use tracing::warn;

/// ALPN for clanker-router HTTP tunnel over iroh.
pub const ALPN: &[u8] = b"clanker-router-http/1";

/// Metrics for the iroh tunnel.
#[derive(Debug, Default)]
struct TunnelMetrics {
    connections_accepted: AtomicU64,
    streams_total: AtomicU64,
    streams_active: AtomicU64,
    streams_completed: AtomicU64,
    streams_failed: AtomicU64,
    bytes_from_peer: AtomicU64,
    bytes_to_peer: AtomicU64,
}

/// Iroh protocol handler that tunnels connections to a local HTTP server.
///
/// Each accepted iroh bidirectional stream is piped to a fresh TCP connection
/// to the local proxy address. The peer speaks standard HTTP/1.1 through the
/// tunnel — the same requests that would go to the TCP proxy port.
#[derive(Debug)]
pub struct IrohTunnel {
    /// Local address to forward connections to.
    target: SocketAddr,
    /// Connection counter for logging.
    conn_id: AtomicU64,
    /// Metrics.
    metrics: Arc<TunnelMetrics>,
}

impl IrohTunnel {
    /// Create a new tunnel that forwards to the given local address.
    pub fn new(target: SocketAddr) -> Arc<Self> {
        Arc::new(Self {
            target,
            conn_id: AtomicU64::new(0),
            metrics: Arc::new(TunnelMetrics::default()),
        })
    }

    /// Number of streams currently being handled.
    pub fn active_streams(&self) -> u64 {
        self.metrics.streams_active.load(Ordering::Relaxed)
    }

    /// Total streams handled since startup.
    pub fn total_streams(&self) -> u64 {
        self.metrics.streams_total.load(Ordering::Relaxed)
    }
}

impl ProtocolHandler for IrohTunnel {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        let conn_id = self.conn_id.fetch_add(1, Ordering::Relaxed);
        let remote = connection.remote_id();
        self.metrics.connections_accepted.fetch_add(1, Ordering::Relaxed);
        info!(
            conn = conn_id,
            remote = %remote.fmt_short(),
            "iroh tunnel: accepted connection"
        );

        // Accept bidirectional streams until the connection closes.
        loop {
            let (send, recv) = match connection.accept_bi().await {
                Ok(streams) => streams,
                Err(iroh::endpoint::ConnectionError::ApplicationClosed(_)) => {
                    debug!(conn = conn_id, "iroh tunnel: connection closed by peer");
                    break;
                }
                Err(e) => {
                    debug!(conn = conn_id, "iroh tunnel: connection error: {e}");
                    break;
                }
            };

            let stream_id = self.metrics.streams_total.fetch_add(1, Ordering::Relaxed);
            self.metrics.streams_active.fetch_add(1, Ordering::Relaxed);
            let target = self.target;
            let metrics = Arc::clone(&self.metrics);

            tokio::spawn(async move {
                debug!(conn = conn_id, stream = stream_id, "iroh tunnel: handling stream → {}", target,);

                let result = tunnel_stream(send, recv, target).await;
                metrics.streams_active.fetch_sub(1, Ordering::Relaxed);

                match result {
                    Ok((to_origin, from_origin)) => {
                        metrics.streams_completed.fetch_add(1, Ordering::Relaxed);
                        metrics.bytes_from_peer.fetch_add(to_origin, Ordering::Relaxed);
                        metrics.bytes_to_peer.fetch_add(from_origin, Ordering::Relaxed);
                        debug!(
                            conn = conn_id,
                            stream = stream_id,
                            to_origin,
                            from_origin,
                            "iroh tunnel: stream complete"
                        );
                    }
                    Err(e) => {
                        metrics.streams_failed.fetch_add(1, Ordering::Relaxed);
                        warn!(conn = conn_id, stream = stream_id, "iroh tunnel: stream error: {e}");
                    }
                }
            });
        }

        Ok(())
    }

    async fn shutdown(&self) {
        let m = &self.metrics;
        info!(
            active = m.streams_active.load(Ordering::Relaxed),
            total = m.streams_total.load(Ordering::Relaxed),
            completed = m.streams_completed.load(Ordering::Relaxed),
            failed = m.streams_failed.load(Ordering::Relaxed),
            "iroh tunnel: shutting down"
        );
    }
}

/// Tunnel a single bidirectional iroh stream to a local TCP connection.
///
/// Returns `(bytes_from_peer, bytes_to_peer)` on success.
async fn tunnel_stream(
    mut iroh_send: iroh::endpoint::SendStream,
    mut iroh_recv: iroh::endpoint::RecvStream,
    target: SocketAddr,
) -> std::io::Result<(u64, u64)> {
    // Connect to the local HTTP server
    let tcp = TcpStream::connect(target).await?;
    let (mut tcp_recv, mut tcp_send) = tcp.into_split();

    // Bidirectional copy: iroh ↔ TCP
    //   iroh_recv → tcp_send  (peer's request → local server)
    //   tcp_recv  → iroh_send (local server's response → peer)
    let (r1, r2) = tokio::join!(
        async {
            let result = tokio::io::copy(&mut iroh_recv, &mut tcp_send).await;
            tcp_send.shutdown().await.ok();
            result
        },
        async {
            let result = tokio::io::copy(&mut tcp_recv, &mut iroh_send).await;
            iroh_send.finish().ok();
            result
        }
    );

    Ok((r1?, r2?))
}
