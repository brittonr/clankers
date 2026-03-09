//! Matrix bridge for daemon mode.
//!
//! Handles Matrix transport: user messages, bot commands, media uploads,
//! and proactive agent features (heartbeats, trigger pipes).

mod allowlist;
mod bot_commands;
mod proactive;
mod prompt;
mod sendfile;

// Internal imports
use std::sync::Arc;

use allowlist::is_user_allowed;
use allowlist::resolve_matrix_allowlist;
use bot_commands::handle_bot_command;
use clankers_auth::Capability;
use proactive::ensure_trigger_pipe;
use proactive::run_session_heartbeat;
use prompt::run_matrix_prompt;
use prompt::run_matrix_prompt_with_images;
use sendfile::extract_sendfile_tags;
use sendfile::guess_mime;
use sendfile::upload_sendfiles;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;
use tracing::warn;

use super::daemon::ProactiveConfig;
use super::daemon::SessionKey;
use super::daemon::SessionStore;
use crate::config::ClankersPaths;
use crate::error::Result;
use crate::provider::message::Content;
use crate::provider::message::ImageSource;

pub(crate) async fn run_matrix_bridge(
    store: Arc<RwLock<SessionStore>>,
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

    // Resolve allowlist
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

    // ── Heartbeat scheduler ─────────────────────────────────────────
    if proactive.session_heartbeat_secs > 0 {
        let hb_store = Arc::clone(&store);
        let hb_client = Arc::clone(&client);
        let hb_cancel = cancel.clone();
        let hb_interval = std::time::Duration::from_secs(proactive.session_heartbeat_secs);
        let hb_prompt = proactive.heartbeat_prompt.clone();
        tokio::spawn(async move {
            run_session_heartbeat(hb_store, hb_client, hb_interval, hb_prompt, hb_cancel).await;
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
                        // ── Auth check: token → allowlist fallback ──
                        let sender_capabilities: Option<Vec<Capability>> = {
                            let store_guard = store.read().await;
                            if let Some(ref auth) = store_guard.auth {
                                match auth.resolve_capabilities(&sender) {
                                    Some(Ok(caps)) => Some(caps),
                                    Some(Err(e)) => {
                                        // Token exists but is invalid (expired, revoked, etc.)
                                        warn!("Matrix: token error for {}: {e}", sender);
                                        let room_id_parsed = clankers_matrix::ruma::RoomId::parse(&room_id).ok();
                                        if let Some(rid) = room_id_parsed {
                                            let c = client.read().await;
                                            let msg = format!(
                                                "Your access token is invalid: {e}\n\
                                                 Request a new one from the daemon owner, \
                                                 or register with `!token <base64>`."
                                            );
                                            let _ = c.send_markdown(&rid, &msg).await;
                                        }
                                        continue;
                                    }
                                    None => {
                                        // No token — fall back to allowlist
                                        if !is_user_allowed(&allowlist, &sender) {
                                            info!("Matrix: denied message from {} (no token, not on allowlist)", sender);
                                            continue;
                                        }
                                        None // Full access via allowlist
                                    }
                                }
                            } else {
                                // No auth layer — use allowlist only
                                if !is_user_allowed(&allowlist, &sender) {
                                    info!("Matrix: denied message from {}", sender);
                                    continue;
                                }
                                None
                            }
                        };

                        // ── Skip client slash commands ──────────────
                        if body.starts_with('/') {
                            continue;
                        }

                        // ── Bot command dispatch ────────────────────
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
                                &body,
                                &key,
                                Arc::clone(&store),
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

                        // ── Typing indicator: start ─────────────────
                        {
                            let c = client.read().await;
                            if let Err(e) = c.set_typing(&room_id_parsed, true).await {
                                warn!("Typing indicator start failed: {e}");
                            }
                        }

                        // Spawn a typing refresh task (re-sends every 25s)
                        let typing_cancel = CancellationToken::new();
                        let typing_client = Arc::clone(&client);
                        let typing_room = room_id_parsed.clone();
                        let typing_token = typing_cancel.clone();
                        tokio::spawn(async move {
                            let mut interval = tokio::time::interval(std::time::Duration::from_secs(25));
                            interval.tick().await; // skip the immediate tick
                            loop {
                                tokio::select! {
                                    _ = interval.tick() => {
                                        let c = typing_client.read().await;
                                        let _ = c.set_typing(&typing_room, true).await;
                                    }
                                    () = typing_token.cancelled() => break,
                                }
                            }
                        });

                        // ── Run prompt ──────────────────────────────
                        let mut response = run_matrix_prompt(
                            Arc::clone(&store), key, body,
                            sender_capabilities.as_deref(),
                        ).await;

                        // ── Empty response re-prompt ────────────────
                        if response.trim().is_empty() {
                            info!("Empty response, re-prompting for summary");
                            let re_key = SessionKey::Matrix {
                                user_id: sender.clone(),
                                room_id: room_id.clone(),
                            };
                            let retry = run_matrix_prompt(
                                Arc::clone(&store),
                                re_key,
                                "You completed some actions but your response contained \
                                 no text. Briefly summarize what you did.".to_string(),
                                sender_capabilities.as_deref(),
                            ).await;
                            if retry.trim().is_empty() {
                                response = "(completed actions — no summary available)".to_string();
                            } else {
                                response = retry;
                            }
                        }

                        // ── Typing indicator: stop ──────────────────
                        typing_cancel.cancel();
                        {
                            let c = client.read().await;
                            let _ = c.set_typing(&room_id_parsed, false).await;
                        }

                        // ── Sendfile extraction + upload ─────────────
                        let (cleaned, sendfiles) = extract_sendfile_tags(&response);
                        response = cleaned;

                        if !sendfiles.is_empty() {
                            let errors = upload_sendfiles(&client, &room_id_parsed, &sendfiles).await;
                            for err in errors {
                                response.push('\n');
                                response.push_str(&err);
                            }
                        }

                        // ── Send response ───────────────────────────
                        let c = client.read().await;
                        if let Err(e) = c.send_markdown(&room_id_parsed, &response).await {
                            error!("Matrix send failed: {e}");
                        }

                        // ── Trigger pipe setup ──────────────────────
                        if proactive.trigger_pipe_enabled {
                            let tp_key = SessionKey::Matrix {
                                user_id: sender.clone(),
                                room_id: room_id.clone(),
                            };
                            ensure_trigger_pipe(
                                Arc::clone(&store),
                                &tp_key,
                                Arc::clone(&client),
                            )
                            .await;
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
                        // ── Auth check: token → allowlist fallback ──
                        let sender_capabilities: Option<Vec<Capability>> = {
                            let store_guard = store.read().await;
                            if let Some(ref auth) = store_guard.auth {
                                match auth.resolve_capabilities(&sender) {
                                    Some(Ok(caps)) => Some(caps),
                                    Some(Err(_)) => {
                                        info!("Matrix: denied media from {} (invalid token)", sender);
                                        continue;
                                    }
                                    None => {
                                        if !is_user_allowed(&allowlist, &sender) {
                                            info!("Matrix: denied media from {}", sender);
                                            continue;
                                        }
                                        None
                                    }
                                }
                            } else {
                                if !is_user_allowed(&allowlist, &sender) {
                                    info!("Matrix: denied media from {}", sender);
                                    continue;
                                }
                                None
                            }
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

                        // ── Typing indicator: start ─────────────────
                        {
                            let c = client.read().await;
                            if let Err(e) = c.set_typing(&room_id_parsed, true).await {
                                warn!("Typing indicator start failed: {e}");
                            }
                        }

                        let typing_cancel = CancellationToken::new();
                        let typing_client = Arc::clone(&client);
                        let typing_room = room_id_parsed.clone();
                        let typing_token = typing_cancel.clone();
                        tokio::spawn(async move {
                            let mut interval = tokio::time::interval(std::time::Duration::from_secs(25));
                            interval.tick().await;
                            loop {
                                tokio::select! {
                                    _ = interval.tick() => {
                                        let c = typing_client.read().await;
                                        let _ = c.set_typing(&typing_room, true).await;
                                    }
                                    () = typing_token.cancelled() => break,
                                }
                            }
                        });

                        // ── Download the attachment ─────────────────
                        let download_result = {
                            let c = client.read().await;
                            c.download_media(&source).await
                        };

                        let file_bytes = match download_result {
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

                        // ── Save to session attachments dir ─────────
                        let attachments_dir = paths
                            .global_sessions_dir
                            .join(format!(
                                "matrix_{}_{}",
                                sender.replace(':', "_"),
                                room_id.replace(':', "_")
                            ))
                            .join("attachments");
                        if let Err(e) = std::fs::create_dir_all(&attachments_dir) {
                            error!("Failed to create attachments dir: {e}");
                        }

                        let save_path = attachments_dir.join(&filename);
                        if let Err(e) = std::fs::write(&save_path, &file_bytes) {
                            error!("Failed to save attachment {}: {e}", save_path.display());
                        }

                        // ── Build prompt with image if applicable ───
                        let caption = if body != filename { &body } else { "" };
                        let is_image = media_type == "image";

                        let mut response = if is_image {
                            // Pass image as base64 content block for vision
                            let b64 = base64::Engine::encode(
                                &base64::engine::general_purpose::STANDARD,
                                &file_bytes,
                            );
                            let mime_str = guess_mime(&save_path).to_string();

                            let image_content = Content::Image {
                                source: ImageSource::Base64 {
                                    media_type: mime_str,
                                    data: b64,
                                },
                            };

                            let prompt_text = if caption.is_empty() {
                                format!(
                                    "User sent an image: {} (saved to {})",
                                    filename,
                                    save_path.display()
                                )
                            } else {
                                format!(
                                    "User sent an image with caption \"{}\": {} (saved to {})",
                                    caption, filename, save_path.display()
                                )
                            };

                            run_matrix_prompt_with_images(
                                Arc::clone(&store),
                                key,
                                prompt_text,
                                vec![image_content],
                                sender_capabilities.as_deref(),
                            )
                            .await
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

                            run_matrix_prompt(Arc::clone(&store), key, prompt_text, sender_capabilities.as_deref()).await
                        };

                        // ── Empty response re-prompt ────────────────
                        if response.trim().is_empty() {
                            let re_key = SessionKey::Matrix {
                                user_id: sender.clone(),
                                room_id: room_id.clone(),
                            };
                            let retry = run_matrix_prompt(
                                Arc::clone(&store),
                                re_key,
                                "You processed a file but your response contained no text. \
                                 Briefly summarize what you did."
                                    .to_string(),
                                sender_capabilities.as_deref(),
                            )
                            .await;
                            response = if retry.trim().is_empty() {
                                "(processed file — no summary available)".to_string()
                            } else {
                                retry
                            };
                        }

                        // ── Sendfile extraction + upload ────────────
                        let (cleaned, sendfiles) = extract_sendfile_tags(&response);
                        response = cleaned;

                        if !sendfiles.is_empty() {
                            let errors = upload_sendfiles(&client, &room_id_parsed, &sendfiles).await;
                            for err in errors {
                                response.push('\n');
                                response.push_str(&err);
                            }
                        }

                        // ── Typing indicator: stop ──────────────────
                        typing_cancel.cancel();
                        {
                            let c = client.read().await;
                            let _ = c.set_typing(&room_id_parsed, false).await;
                        }

                        // ── Send response ───────────────────────────
                        let c = client.read().await;
                        if let Err(e) = c.send_markdown(&room_id_parsed, &response).await {
                            error!("Matrix send failed: {e}");
                        }

                        // ── Trigger pipe setup ──────────────────────
                        if proactive.trigger_pipe_enabled {
                            let tp_key = SessionKey::Matrix {
                                user_id: sender.clone(),
                                room_id: room_id.clone(),
                            };
                            ensure_trigger_pipe(
                                Arc::clone(&store),
                                &tp_key,
                                Arc::clone(&client),
                            )
                            .await;
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
