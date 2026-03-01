//! Zellij session streaming — host side
//!
//! Shares a zellij session by proxying the Unix socket over iroh QUIC.

use std::path::Path;
use std::path::PathBuf;

use iroh::Endpoint;
use iroh::SecretKey;
use tokio::net::UnixStream;

use super::handshake;
use super::protocol::ALPN;
use super::protocol::SessionInfo;
use super::protocol::{self};

/// Locate the zellij session socket
pub fn find_session_socket(session_name: &str) -> Option<PathBuf> {
    // Linux: ~/.local/share/zellij/ or /tmp/zellij-{uid}/
    // Get UID from id command or use fallback
    let uid = std::process::Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(1000); // fallback UID

    // Try XDG first
    if let Ok(data_dir) = std::env::var("XDG_RUNTIME_DIR") {
        let path = PathBuf::from(data_dir).join("zellij").join(session_name);
        if path.exists() {
            return Some(path);
        }
    }

    // Try /tmp/zellij-{uid}/
    let path = PathBuf::from(format!("/tmp/zellij-{}/{}", uid, session_name));
    if path.exists() {
        return Some(path);
    }

    // Try ~/.local/share/zellij/
    if let Some(home) = dirs::home_dir() {
        let path = home.join(".local/share/zellij").join(session_name);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Host a zellij session over iroh
pub async fn host_session(
    session_name: &str,
    secret_key: SecretKey,
    read_only: bool,
) -> Result<(Endpoint, [u8; 32]), crate::error::Error> {
    let psk = handshake::generate_psk();

    let endpoint = Endpoint::builder().secret_key(secret_key).alpns(vec![ALPN.to_vec()]).bind().await.map_err(|e| {
        crate::error::Error::Zellij {
            message: format!("Failed to bind endpoint: {}", e),
        }
    })?;

    let socket_path = find_session_socket(session_name).ok_or_else(|| crate::error::Error::Zellij {
        message: format!("Zellij session socket not found: {}", session_name),
    })?;

    let session_info = SessionInfo {
        session_name: session_name.to_string(),
        zellij_version: detect_zellij_version(),
        read_only,
    };

    let ep = endpoint.clone();
    let psk_copy = psk;
    tokio::spawn(async move {
        accept_guests(ep, &socket_path, &psk_copy, &session_info).await;
    });

    Ok((endpoint, psk))
}

async fn accept_guests(endpoint: Endpoint, socket_path: &Path, psk: &[u8; 32], session_info: &SessionInfo) {
    loop {
        let incoming = match endpoint.accept().await {
            Some(i) => i,
            None => break,
        };

        let socket_path = socket_path.to_path_buf();
        let psk = *psk;
        let info = session_info.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_guest(incoming, &socket_path, &psk, &info).await {
                tracing::warn!("Guest connection error: {}", e);
            }
        });
    }
}

async fn handle_guest(
    incoming: iroh::endpoint::Incoming,
    socket_path: &Path,
    psk: &[u8; 32],
    info: &SessionInfo,
) -> Result<(), crate::error::Error> {
    let conn = incoming.await.map_err(|e| crate::error::Error::Zellij {
        message: format!("Connection failed: {}", e),
    })?;

    // Open control stream for handshake
    let (mut send, mut recv) = conn.accept_bi().await.map_err(|e| crate::error::Error::Zellij {
        message: format!("Failed to accept stream: {}", e),
    })?;

    // Verify PSK
    if !handshake::verify_psk(&mut recv, psk).await.map_err(|e| crate::error::Error::Zellij {
        message: format!("PSK verification failed: {}", e),
    })? {
        tracing::warn!("Invalid PSK from guest");
        return Ok(());
    }

    // Send session info
    protocol::write_message(&mut send, info).await.map_err(|e| crate::error::Error::Zellij {
        message: format!("Failed to send session info: {}", e),
    })?;

    // Open data stream and proxy to zellij socket
    let (mut quic_send, mut quic_recv) = conn.accept_bi().await.map_err(|e| crate::error::Error::Zellij {
        message: format!("Failed to accept data stream: {}", e),
    })?;

    let unix_stream = UnixStream::connect(socket_path).await.map_err(|e| crate::error::Error::Zellij {
        message: format!("Failed to connect to zellij socket: {}", e),
    })?;

    let (mut unix_read, mut unix_write) = unix_stream.into_split();

    // Bidirectional proxy
    let a = tokio::spawn(async move {
        let _ = tokio::io::copy(&mut quic_recv, &mut unix_write).await;
    });
    let b = tokio::spawn(async move {
        let _ = tokio::io::copy(&mut unix_read, &mut quic_send).await;
    });

    let _ = tokio::try_join!(a, b);
    Ok(())
}

fn detect_zellij_version() -> String {
    std::process::Command::new("zellij")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string())
        .trim()
        .to_string()
}
