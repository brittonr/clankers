//! Matrix bridge for daemon mode.
//!
//! Handles Matrix transport: user messages, bot commands, media uploads,
//! and proactive agent features (heartbeats, trigger pipes).
//!
//! All sessions are backed by actor processes via `get_or_create_keyed_session`.

mod allowlist;
mod bot_commands;
mod proactive;
mod prompt;
mod sendfile;

use std::sync::Arc;

use allowlist::is_user_allowed;
use allowlist::resolve_matrix_allowlist;
use bot_commands::handle_bot_command;
use clanker_actor::ProcessRegistry;
use clankers_controller::transport::DaemonState;
use clankers_protocol::SessionKey;
use proactive::ensure_trigger_pipe;
use proactive::run_session_heartbeat;
use prompt::run_matrix_prompt;
use prompt::run_matrix_prompt_with_images;
use sendfile::extract_sendfile_tags;
use sendfile::guess_mime;
use sendfile::upload_sendfiles;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;
use tracing::warn;

use super::daemon::session_store::AuthLayer;
use super::daemon::socket_bridge::SessionFactory;
use super::daemon::ProactiveConfig;
use crate::config::ClankersPaths;
use crate::error::Result;

pub(crate) async fn run_matrix_bridge(
    state: Arc<Mutex<DaemonState>>,
    registry: ProcessRegistry,
    factory: Arc<SessionFactory>,
    auth: Option<Arc<AuthLayer>>,
    cancel: CancellationToken,
    paths: &ClankersPaths,
    daemon_allowed_users: Vec<String>,
    proactive: ProactiveConfig,
) -> Result<()> {
    use clankers_matrix::MatrixConfig;
    use clankers_matrix::bridge::BridgeEvent;
    use clankers_matrix::bridge::MatrixBridge;

    let config_path = paths.global_config_dir.join("matrix.json");
    let config = MatrixConfig::load(&config_path).ok_or_else(|| crate::error::Error::Config {
        message: format!("Matrix config not found at {}. Run `clankers matrix login` first.", config_path.display()),
    })?;

    let allowlist = resolve_matrix_allowlist(&config, &daemon_allowed_users);
    if allowlist.is_empty() {
        info!("Matrix allowlist: open (all users accepted)");
    } else {
        info!("Matrix allowlist: {} user(s)", allowlist.len());
    }

    let store_path = config.resolve_store_path(&paths.global_config_dir);
    let mut client = clankers_matrix::MatrixClient::new(config, "clankers-daemon");

    client.restore_session(&store_path).await.map_err(|e| crate::error::Error::Provider {
        message: format!("Matrix session restore failed: {e}"),
    })?;

    client.start_sync().await.map_err(|e| crate::error::Error::Provider {
        message: format!("Matrix sync failed: {e}"),
    })?;

    let our_user_id = client.user_id().map(|u| u.to_string()).unwrap_or_default();
    let event_rx = client.subscribe();
    let client = Arc::new(tokio::sync::RwLock::new(client));

    let mut bridge = MatrixBridge::new();
    let mut agent_rx = bridge.take_event_rx().expect("first call");
    bridge.start(event_rx, &our_user_id);

    info!("Matrix bridge started for {}", our_user_id);

    let sessions_dir = paths.global_sessions_dir.clone();

    // Heartbeat scheduler
    if proactive.session_heartbeat_secs > 0 {
        let hb_state = Arc::clone(&state);
        let hb_registry = registry.clone();
        let hb_factory = Arc::clone(&factory);
        let hb_sessions_dir = sessions_dir.clone();
        let hb_client = Arc::clone(&client);
        let hb_cancel = cancel.clone();
        let hb_interval = std::time::Duration::from_secs(proactive.session_heartbeat_secs);
        let hb_prompt = proactive.heartbeat_prompt.clone();
        tokio::spawn(async move {
            run_session_heartbeat(
                hb_state, hb_registry, hb_factory, hb_sessions_dir,
                hb_client, hb_interval, hb_prompt, hb_cancel,
            ).await;
        });
        info!("Session heartbeat scheduler started (interval: {}s)", proactive.session_heartbeat_secs);
    }

    loop {
        tokio::select! {
            event = agent_rx.recv() => {
                let Some(event) = event else { break };
                match event {
                    BridgeEvent::TextMessage { sender, body, room_id }
                    | BridgeEvent::ChatMessage { sender, body, room_id, .. } => {
                        // Auth check — extract capabilities from token
                        let sender_caps = match check_sender_auth(
                            &sender, &auth, &allowlist, &client, &room_id,
                        ).await {
                            SendCheckResult::Allowed(caps) => caps,
                            SendCheckResult::Denied => continue,
                        };

                        if body.starts_with('/') {
                            continue;
                        }

                        // Bot command dispatch
                        if body.starts_with('!') {
                            let room_id_parsed = match clankers_matrix::ruma::RoomId::parse(&room_id) {
                                Ok(rid) => rid.clone(),
                                Err(_) => continue,
                            };
                            let key = SessionKey::Matrix {
                                user_id: sender.clone(),
                                room_id: room_id.clone(),
                            };
                            let response = handle_bot_command(
                                &body, &key,
                                Arc::clone(&state),
                                registry.clone(),
                                Arc::clone(&factory),
                                auth.clone(),
                            ).await;
                            let c = client.read().await;
                            if let Err(e) = c.send_markdown(&room_id_parsed, &response).await {
                                error!("Matrix send failed: {e}");
                            }
                            continue;
                        }

                        let key = SessionKey::Matrix {
                            user_id: sender.clone(),
                            room_id: room_id.clone(),
                        };

                        info!("[{}] message: {}", key, &body[..80.min(body.len())]);

                        let room_id_parsed = match clankers_matrix::ruma::RoomId::parse(&room_id) {
                            Ok(rid) => rid.clone(),
                            Err(_) => continue,
                        };

                        // Typing indicator
                        {
                            let c = client.read().await;
                            let _ = c.set_typing(&room_id_parsed, true).await;
                        }
                        let typing_cancel = spawn_typing_refresh(Arc::clone(&client), room_id_parsed.clone());

                        // Run prompt (with UCAN capabilities for tool enforcement)
                        let mut response = run_matrix_prompt(
                            Arc::clone(&state),
                            registry.clone(),
                            Arc::clone(&factory),
                            key.clone(),
                            body,
                            sender_caps.clone(),
                        ).await;

                        // Empty response re-prompt
                        if response.trim().is_empty() {
                            info!("Empty response, re-prompting for summary");
                            let retry = run_matrix_prompt(
                                Arc::clone(&state),
                                registry.clone(),
                                Arc::clone(&factory),
                                key.clone(),
                                "You completed some actions but your response contained \
                                 no text. Briefly summarize what you did.".to_string(),
                                sender_caps.clone(),
                            ).await;
                            response = if retry.trim().is_empty() {
                                "(completed actions — no summary available)".to_string()
                            } else {
                                retry
                            };
                        }

                        // Stop typing
                        typing_cancel.cancel();
                        {
                            let c = client.read().await;
                            let _ = c.set_typing(&room_id_parsed, false).await;
                        }

                        // Sendfile extraction + upload
                        let (cleaned, sendfiles) = extract_sendfile_tags(&response);
                        response = cleaned;
                        if !sendfiles.is_empty() {
                            let errors = upload_sendfiles(&client, &room_id_parsed, &sendfiles).await;
                            for err in errors {
                                response.push('\n');
                                response.push_str(&err);
                            }
                        }

                        // Send response
                        let c = client.read().await;
                        if let Err(e) = c.send_markdown(&room_id_parsed, &response).await {
                            error!("Matrix send failed: {e}");
                        }

                        // Trigger pipe setup
                        if proactive.trigger_pipe_enabled {
                            ensure_trigger_pipe(
                                Arc::clone(&state),
                                registry.clone(),
                                Arc::clone(&factory),
                                &key,
                                &sessions_dir,
                                Arc::clone(&client),
                            ).await;
                        }
                    }
                    BridgeEvent::MediaMessage {
                        sender,
                        body,
                        filename,
                        media_type,
                        source,
                        room_id,
                    } => {
                        let media_caps = match check_sender_auth(
                            &sender, &auth, &allowlist, &client, &room_id,
                        ).await {
                            SendCheckResult::Allowed(caps) => caps,
                            SendCheckResult::Denied => continue,
                        };

                        let key = SessionKey::Matrix {
                            user_id: sender.clone(),
                            room_id: room_id.clone(),
                        };

                        info!("[{}] media: {} ({})", key, filename, media_type);

                        let room_id_parsed = match clankers_matrix::ruma::RoomId::parse(&room_id) {
                            Ok(rid) => rid.clone(),
                            Err(_) => continue,
                        };

                        {
                            let c = client.read().await;
                            let _ = c.set_typing(&room_id_parsed, true).await;
                        }
                        let typing_cancel = spawn_typing_refresh(Arc::clone(&client), room_id_parsed.clone());

                        // Download the attachment
                        let file_bytes = match client.read().await.download_media(&source).await {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                error!("Failed to download media {}: {e}", filename);
                                typing_cancel.cancel();
                                let c = client.read().await;
                                let _ = c.set_typing(&room_id_parsed, false).await;
                                let _ = c.send_markdown(&room_id_parsed, &format!(
                                    "Failed to download attachment: {e}"
                                )).await;
                                continue;
                            }
                        };

                        // Save to session attachments dir
                        let attachments_dir = sessions_dir
                            .join(key.dir_name())
                            .join("attachments");
                        let _ = std::fs::create_dir_all(&attachments_dir);
                        let save_path = attachments_dir.join(&filename);
                        if let Err(e) = std::fs::write(&save_path, &file_bytes) {
                            error!("Failed to save attachment {}: {e}", save_path.display());
                        }

                        let caption = if body != filename { &body } else { "" };
                        let is_image = media_type == "image";

                        let mut response = if is_image {
                            let b64 = base64::Engine::encode(
                                &base64::engine::general_purpose::STANDARD,
                                &file_bytes,
                            );
                            let mime_str = guess_mime(&save_path).to_string();
                            let image_data = clankers_protocol::ImageData {
                                data: b64,
                                media_type: mime_str,
                            };

                            let prompt_text = if caption.is_empty() {
                                format!(
                                    "User sent an image: {} (saved to {})",
                                    filename, save_path.display()
                                )
                            } else {
                                format!(
                                    "User sent an image with caption \"{}\": {} (saved to {})",
                                    caption, filename, save_path.display()
                                )
                            };

                            run_matrix_prompt_with_images(
                                Arc::clone(&state),
                                registry.clone(),
                                Arc::clone(&factory),
                                key.clone(),
                                prompt_text,
                                vec![image_data],
                                media_caps.clone(),
                            ).await
                        } else {
                            let prompt_text = if caption.is_empty() {
                                format!(
                                    "User sent a {} file: {} (saved to {})",
                                    media_type, filename, save_path.display()
                                )
                            } else {
                                format!(
                                    "User sent a {} file with caption \"{}\": {} (saved to {})",
                                    media_type, caption, filename, save_path.display()
                                )
                            };

                            run_matrix_prompt(
                                Arc::clone(&state),
                                registry.clone(),
                                Arc::clone(&factory),
                                key.clone(),
                                prompt_text,
                                media_caps.clone(),
                            ).await
                        };

                        // Empty response re-prompt
                        if response.trim().is_empty() {
                            let retry = run_matrix_prompt(
                                Arc::clone(&state),
                                registry.clone(),
                                Arc::clone(&factory),
                                key.clone(),
                                "You processed a file but your response contained no text. \
                                 Briefly summarize what you did.".to_string(),
                                media_caps.clone(),
                            ).await;
                            response = if retry.trim().is_empty() {
                                "(processed file — no summary available)".to_string()
                            } else {
                                retry
                            };
                        }

                        let (cleaned, sendfiles) = extract_sendfile_tags(&response);
                        response = cleaned;
                        if !sendfiles.is_empty() {
                            let errors = upload_sendfiles(&client, &room_id_parsed, &sendfiles).await;
                            for err in errors {
                                response.push('\n');
                                response.push_str(&err);
                            }
                        }

                        typing_cancel.cancel();
                        {
                            let c = client.read().await;
                            let _ = c.set_typing(&room_id_parsed, false).await;
                        }

                        let c = client.read().await;
                        if let Err(e) = c.send_markdown(&room_id_parsed, &response).await {
                            error!("Matrix send failed: {e}");
                        }

                        if proactive.trigger_pipe_enabled {
                            ensure_trigger_pipe(
                                Arc::clone(&state),
                                registry.clone(),
                                Arc::clone(&factory),
                                &key,
                                &sessions_dir,
                                Arc::clone(&client),
                            ).await;
                        }
                    }
                    BridgeEvent::PeerUpdate(peer) => {
                        info!("Matrix peer update: {} ({})", peer.instance_name, peer.user_id);
                    }
                    _ => {}
                }
            }
            () = cancel.cancelled() => break,
        }
    }

    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────────────────

enum SendCheckResult {
    /// Allowed with optional UCAN capabilities (None = full access)
    Allowed(Option<Vec<clankers_ucan::Capability>>),
    Denied,
}

/// Check auth for a Matrix sender: token → allowlist fallback.
///
/// Returns `Allowed(capabilities)` where capabilities are `Some` if the
/// sender has a UCAN token, or `None` for allowlist-only users (full access).
async fn check_sender_auth(
    sender: &str,
    auth: &Option<Arc<AuthLayer>>,
    allowlist: &[String],
    client: &tokio::sync::RwLock<clankers_matrix::MatrixClient>,
    room_id: &str,
) -> SendCheckResult {
    if let Some(auth) = auth {
        match auth.resolve_capabilities(sender) {
            Some(Ok(caps)) => SendCheckResult::Allowed(Some(caps)),
            Some(Err(e)) => {
                warn!("Matrix: token error for {}: {e}", sender);
                if let Ok(rid) = clankers_matrix::ruma::RoomId::parse(room_id) {
                    let c = client.read().await;
                    let msg = format!(
                        "Your access token is invalid: {e}\n\
                         Request a new one from the daemon owner, \
                         or register with `!token <base64>`."
                    );
                    let _ = c.send_markdown(&rid, &msg).await;
                }
                SendCheckResult::Denied
            }
            None => {
                if is_user_allowed(allowlist, sender) {
                    SendCheckResult::Allowed(None) // allowlist users get full access
                } else {
                    info!("Matrix: denied message from {} (no token, not on allowlist)", sender);
                    SendCheckResult::Denied
                }
            }
        }
    } else if is_user_allowed(allowlist, sender) {
        SendCheckResult::Allowed(None) // no auth layer = full access
    } else {
        info!("Matrix: denied message from {}", sender);
        SendCheckResult::Denied
    }
}

/// Spawn a typing indicator refresh task (re-sends every 25s).
fn spawn_typing_refresh(
    client: Arc<tokio::sync::RwLock<clankers_matrix::MatrixClient>>,
    room_id: clankers_matrix::ruma::OwnedRoomId,
) -> CancellationToken {
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(25));
        interval.tick().await;
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let c = client.read().await;
                    let _ = c.set_typing(&room_id, true).await;
                }
                () = token.cancelled() => break,
            }
        }
    });
    cancel
}
