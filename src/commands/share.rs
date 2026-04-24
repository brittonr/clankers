//! Share and join Zellij session handlers.

use crate::error::Result;

/// Share the current Zellij session over the network.
///
/// Creates an iroh endpoint and hosts the session, providing credentials
/// that remote users can use to join with `clankers join`.
pub async fn run_share(_ctx: &crate::commands::CommandContext, is_read_only: bool) -> Result<()> {
    let session_name = crate::zellij::session_name().ok_or_else(|| crate::error::Error::Config {
        message: "Not inside a Zellij session. Start clankers inside Zellij first, or use: clankers --zellij"
            .to_string(),
    })?;

    println!("Sharing Zellij session: {}", session_name);
    let secret_key = iroh::SecretKey::generate(&mut rand::rng());
    let node_id = secret_key.public();

    let (_endpoint, psk) =
        crate::zellij::streaming::host::host_session(&session_name, secret_key, is_read_only).await?;
    let psk_hex = crate::zellij::streaming::handshake::psk_to_hex(&psk);
    println!("\nSession shared! Give the remote user these credentials:\n");
    println!("  clankers join {} {}\n", node_id, psk_hex);
    println!("Press Ctrl+C to stop sharing.");
    tokio::signal::ctrl_c().await.ok();
    println!("\nStopped sharing.");
    Ok(())
}

/// Join a remote shared Zellij session.
///
/// Connects to a remote session using the node ID and pre-shared key
/// provided by the host.
pub async fn run_join(node_id: &str, psk: &str) -> Result<()> {
    let node_id: iroh::EndpointId = node_id.parse().map_err(|e| crate::error::Error::Config {
        message: format!("Invalid node ID: {}", e),
    })?;

    let psk_bytes =
        crate::zellij::streaming::handshake::psk_from_hex(psk).ok_or_else(|| crate::error::Error::Config {
            message: "Invalid PSK (expected 64-char hex string)".to_string(),
        })?;

    let info = crate::zellij::streaming::guest::join_session(node_id, &psk_bytes).await?;
    println!("Connected to session: {}", info.session_name);
    println!("Read-only: {}", info.read_only);
    println!("\nRun this to attach:\n  zellij attach {}-remote\n", info.session_name);
    println!("Press Ctrl+C to disconnect.");
    tokio::signal::ctrl_c().await.ok();
    println!("\nDisconnected.");
    Ok(())
}
