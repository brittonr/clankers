//! Zellij session streaming — guest side
//!
//! Joins a remote zellij session by creating a fake Unix socket
//! and proxying it over iroh QUIC.

use std::path::PathBuf;

use iroh::Endpoint;
use iroh::EndpointAddr;
use iroh::EndpointId;
use iroh::SecretKey;
use tokio::net::UnixListener;

use super::handshake;
use super::protocol::ALPN;
use super::protocol::SessionInfo;
use super::protocol::{self};

/// Join a remote zellij session
pub async fn join_session(node_id: EndpointId, psk: &[u8; 32]) -> Result<SessionInfo, crate::ZellijError> {
    let secret_key = SecretKey::generate(&mut rand::rng());
    let endpoint = Endpoint::builder().secret_key(secret_key).alpns(vec![ALPN.to_vec()]).bind().await.map_err(|e| {
        crate::ZellijError {
            message: format!("Failed to bind endpoint: {}", e),
        }
    })?;

    let node_addr = EndpointAddr::new(node_id);
    let conn = endpoint.connect(node_addr, ALPN).await.map_err(|e| crate::ZellijError {
        message: format!("Failed to connect to host: {}", e),
    })?;

    // Control stream: send PSK, receive session info
    let (mut send, mut recv) = conn.open_bi().await.map_err(|e| crate::ZellijError {
        message: format!("Failed to open control stream: {}", e),
    })?;

    handshake::send_psk(&mut send, psk).await.map_err(|e| crate::ZellijError {
        message: format!("Failed to send PSK: {}", e),
    })?;

    let session_info: SessionInfo = protocol::read_message(&mut recv).await.map_err(|e| crate::ZellijError {
        message: format!("Failed to read session info: {}", e),
    })?;

    // Create fake Unix socket for local zellij client
    let socket_path = create_fake_socket(&session_info)?;

    // Open data stream
    let (quic_send, quic_recv) = conn.open_bi().await.map_err(|e| crate::ZellijError {
        message: format!("Failed to open data stream: {}", e),
    })?;

    // Spawn socket proxy
    let info = session_info.clone();
    tokio::spawn(async move {
        proxy_socket(socket_path, quic_send, quic_recv).await;
    });

    println!("Session ready. Run: zellij attach {}-remote", info.session_name);

    Ok(session_info)
}

fn create_fake_socket(info: &SessionInfo) -> Result<PathBuf, crate::ZellijError> {
    // Try to get UID from environment or use a fallback
    let uid_suffix = std::process::Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(1000); // fallback UID

    let dir = PathBuf::from(format!("/tmp/zellij-{}", uid_suffix));
    std::fs::create_dir_all(&dir).ok();
    let socket_path = dir.join(format!("{}-remote", info.session_name));
    // Remove old socket if it exists
    std::fs::remove_file(&socket_path).ok();
    Ok(socket_path)
}

async fn proxy_socket(
    socket_path: PathBuf,
    mut quic_send: iroh::endpoint::SendStream,
    mut quic_recv: iroh::endpoint::RecvStream,
) {
    let listener = match UnixListener::bind(&socket_path) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind fake socket: {}", e);
            return;
        }
    };

    // Accept one client (the local zellij attach)
    let (unix_stream, _) = match listener.accept().await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to accept local client: {}", e);
            return;
        }
    };

    let (mut unix_read, mut unix_write) = unix_stream.into_split();

    let a = tokio::spawn(async move {
        tokio::io::copy(&mut quic_recv, &mut unix_write).await.ok();
    });
    let b = tokio::spawn(async move {
        tokio::io::copy(&mut unix_read, &mut quic_send).await.ok();
    });

    tokio::try_join!(a, b).ok();
    std::fs::remove_file(&socket_path).ok();
}
