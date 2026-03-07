//! Matrix bridge for daemon mode.
//!
//! Handles Matrix transport: user messages, bot commands, media uploads,
//! and proactive agent features (heartbeats, trigger pipes).

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::agent::events::AgentEvent;
use crate::config::ClankersPaths;
use crate::error::Result;
use crate::provider::message::{Content, ImageSource};
use crate::provider::streaming::ContentDelta;

use clankers_auth::Capability;

use super::daemon::{ProactiveConfig, SessionKey, SessionStore};

/// Resolve the Matrix user allowlist from (in priority order):
/// 1. `CLANKERS_MATRIX_ALLOWED_USERS` env var (comma-separated)
/// 2. `allowed_users` from `matrix.json`
/// 3. `matrix_allowed_users` from `DaemonConfig`
///
/// Empty = allow all.
pub(crate) fn resolve_matrix_allowlist(
    matrix_config: &clankers_matrix::MatrixConfig,
    daemon_allowed: &[String],
) -> Vec<String> {
    if let Ok(env_val) = std::env::var("CLANKERS_MATRIX_ALLOWED_USERS") {
        let users: Vec<String> = env_val.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        if !users.is_empty() {
            return users;
        }
    }
    if !matrix_config.allowed_users.is_empty() {
        return matrix_config.allowed_users.clone();
    }
    daemon_allowed.to_vec()
}

/// Check if a user is in the allowlist (empty = allow all).
pub(crate) fn is_user_allowed(allowlist: &[String], user_id: &str) -> bool {
    allowlist.is_empty() || allowlist.iter().any(|u| u == user_id)
}

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
        info!(
            "Session heartbeat scheduler started (interval: {}s)",
            proactive.session_heartbeat_secs
        );
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

// ── Bot commands ────────────────────────────────────────────────────────────

/// Handle a `!command` from a Matrix user. Returns the response text.
pub(crate) async fn handle_bot_command(
    body: &str,
    key: &SessionKey,
    store: Arc<RwLock<SessionStore>>,
) -> String {
    use clankers_auth::{CapabilityToken, TokenBuilder};

    let parts: Vec<&str> = body.trim().splitn(2, char::is_whitespace).collect();
    let command = parts[0].to_lowercase();
    let args = parts.get(1).unwrap_or(&"").trim();

    match command.as_str() {
        "!help" => {
            "**Available commands:**\n\
             • `!help` — Show this message\n\
             • `!status` — Session info (model, turns, uptime)\n\
             • `!restart` — Clear session history and start fresh\n\
             • `!compact` — Trigger context compaction\n\
             • `!model <name>` — Switch model for this session\n\
             • `!skills` — List loaded skills\n\
             • `!token <base64>` — Register an access token\n\
             • `!delegate [opts]` — Create a child token from yours"
                .to_string()
        }
        "!status" => {
            let store = store.read().await;
            if let Some(session) = store.sessions.get(key) {
                let idle = Utc::now().signed_duration_since(session.last_active);
                let idle_secs = idle.num_seconds().max(0);
                let idle_str = if idle_secs < 60 {
                    format!("{}s", idle_secs)
                } else if idle_secs < 3600 {
                    format!("{}m{}s", idle_secs / 60, idle_secs % 60)
                } else {
                    format!("{}h{}m", idle_secs / 3600, (idle_secs % 3600) / 60)
                };
                format!(
                    "**Session status:**\n\
                     • Model: `{}`\n\
                     • Turns: {}\n\
                     • Idle: {}\n\
                     • Messages in context: {}",
                    store.model,
                    session.turn_count,
                    idle_str,
                    session.agent.messages().len(),
                )
            } else {
                "No active session. Send a message to start one.".to_string()
            }
        }
        "!restart" => {
            let mut store = store.write().await;
            store.sessions.remove(key);
            store.prompt_locks.remove(key);
            "Session cleared. Next message starts a fresh conversation.".to_string()
        }
        "!compact" => {
            // Trigger compaction by running a prompt that asks the agent to compact
            let response = run_matrix_prompt(
                Arc::clone(&store),
                key.clone(),
                "/compact".to_string(),
                None, // compaction uses existing session capabilities
            )
            .await;
            if response.trim().is_empty() {
                "Context compacted.".to_string()
            } else {
                format!("Compaction result: {}", response)
            }
        }
        "!model" => {
            if args.is_empty() {
                let store = store.read().await;
                format!("Current model: `{}`. Usage: `!model <name>`", store.model)
            } else {
                let mut store = store.write().await;
                let old_model = store.model.clone();
                store.model = args.to_string();
                format!("Model switched: `{}` → `{}`", old_model, args)
            }
        }
        "!skills" => {
            let store = store.read().await;
            let tool_names: Vec<String> = store.tools.iter().map(|t| t.definition().name.clone()).collect();
            if tool_names.is_empty() {
                "No tools loaded.".to_string()
            } else {
                format!("**Loaded tools ({}):**\n{}", tool_names.len(), tool_names.iter().map(|n| format!("• `{}`", n)).collect::<Vec<_>>().join("\n"))
            }
        }
        "!token" => {
            if args.is_empty() {
                return "Usage: `!token <base64-encoded-token>`\n\n\
                        Register an access token to get daemon capabilities.\n\
                        Get a token from the daemon owner: `clankers token create`".to_string();
            }

            let store_guard = store.read().await;
            let Some(ref auth) = store_guard.auth else {
                return "Token auth is not enabled on this daemon.".to_string();
            };

            // Decode and verify the token
            let token = match CapabilityToken::from_base64(args) {
                Ok(t) => t,
                Err(e) => return format!("Invalid token: {e}"),
            };

            match auth.verify_token(&token) {
                Ok(caps) => {
                    // Extract the user ID from the session key
                    let user_id = match key {
                        SessionKey::Matrix { user_id, .. } => user_id.clone(),
                        SessionKey::Iroh(id) => id.clone(),
                    };

                    // Store the token mapped to the user ID
                    auth.store_token(&user_id, &token);

                    // Restart the session so it picks up the new capabilities
                    drop(store_guard);
                    {
                        let mut store = store.write().await;
                        store.sessions.remove(key);
                        store.prompt_locks.remove(key);
                    }

                    let cap_names: Vec<&str> = caps.iter().map(|c| match c {
                        Capability::Prompt => "Prompt",
                        Capability::ToolUse { .. } => "ToolUse",
                        Capability::ShellExecute { .. } => "ShellExecute",
                        Capability::FileAccess { .. } => "FileAccess",
                        Capability::BotCommand { .. } => "BotCommand",
                        Capability::SessionManage => "SessionManage",
                        Capability::ModelSwitch => "ModelSwitch",
                        Capability::Delegate => "Delegate",
                    }).collect();

                    let expires = chrono::DateTime::from_timestamp(token.expires_at as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    format!(
                        "**Token accepted** ✓\n\n\
                         • Capabilities: {}\n\
                         • Expires: {}\n\
                         • Depth: {}\n\n\
                         Your session has been restarted with the new capabilities.",
                        cap_names.join(", "),
                        expires,
                        token.delegation_depth,
                    )
                }
                Err(e) => format!("**Token rejected:** {e}"),
            }
        }
        "!delegate" => {
            if args.is_empty() {
                return "**Delegate a child token from yours**\n\n\
                        Usage: `!delegate [options]`\n\n\
                        Options:\n\
                        • `--tools <pattern>` — comma-separated tool names or `*`\n\
                        • `--read-only` — shorthand for `--tools read,grep,find,ls`\n\
                        • `--expire <duration>` — e.g. `1h`, `7d`, `30m`\n\
                        • `--shell` — include ShellExecute capability\n\
                        • `--no-delegate` — child cannot further delegate\n\n\
                        Your token must have the Delegate capability.\n\
                        Child tokens cannot exceed your own permissions."
                    .to_string();
            }

            let store_guard = store.read().await;
            let Some(ref auth) = store_guard.auth else {
                return "Token auth is not enabled on this daemon.".to_string();
            };

            // Look up the sender's token
            let user_id = match key {
                SessionKey::Matrix { user_id, .. } => user_id.clone(),
                SessionKey::Iroh(id) => id.clone(),
            };

            let parent_token = match auth.lookup_token(&user_id) {
                Some(t) => t,
                None => return "You don't have a registered token. Use `!token <base64>` first.".to_string(),
            };

            // Verify the parent token is still valid
            if let Err(e) = auth.verify_token(&parent_token) {
                return format!("Your token is invalid: {e}");
            }

            // Check that the parent has Delegate capability
            if !parent_token.capabilities.contains(&Capability::Delegate) {
                return "Your token does not have the Delegate capability.".to_string();
            }

            // Parse !delegate flags
            let parts: Vec<&str> = args.split_whitespace().collect();
            let mut tools_pattern: Option<String> = None;
            let mut expire_str: Option<&str> = None;
            let mut include_shell = false;
            let mut allow_delegate = true;
            let mut read_only = false;
            let mut i = 0;

            while i < parts.len() {
                match parts[i] {
                    "--tools" => {
                        if i + 1 < parts.len() {
                            tools_pattern = Some(parts[i + 1].to_string());
                            i += 2;
                        } else {
                            return "`--tools` requires a pattern (e.g. `read,grep` or `*`)".to_string();
                        }
                    }
                    "--expire" => {
                        if i + 1 < parts.len() {
                            expire_str = Some(parts[i + 1]);
                            i += 2;
                        } else {
                            return "`--expire` requires a duration (e.g. `1h`, `7d`)".to_string();
                        }
                    }
                    "--shell" => {
                        include_shell = true;
                        i += 1;
                    }
                    "--no-delegate" => {
                        allow_delegate = false;
                        i += 1;
                    }
                    "--read-only" => {
                        read_only = true;
                        i += 1;
                    }
                    other => {
                        return format!("Unknown flag: `{other}`. See `!delegate` for usage.");
                    }
                }
            }

            if read_only {
                tools_pattern = Some("read,grep,find,ls".to_string());
            }

            // Default to 1h if no expiry specified
            let lifetime = match expire_str {
                Some(s) => match parse_delegate_duration(s) {
                    Some(d) => d,
                    None => return format!("Invalid duration: `{s}`. Use e.g. `30m`, `1h`, `7d`, `1y`."),
                },
                None => std::time::Duration::from_secs(3600),
            };

            // Cap child lifetime to parent's remaining lifetime
            let now = clankers_auth::utils::current_time_secs();
            let parent_remaining = parent_token.expires_at.saturating_sub(now);
            let lifetime = lifetime.min(std::time::Duration::from_secs(parent_remaining));

            // Build capabilities for the child token
            let mut child_caps = vec![Capability::Prompt];

            if let Some(pattern) = tools_pattern {
                child_caps.push(Capability::ToolUse {
                    tool_pattern: pattern,
                });
            }

            if include_shell {
                child_caps.push(Capability::ShellExecute {
                    command_pattern: "*".into(),
                    working_dir: None,
                });
            }

            if allow_delegate {
                child_caps.push(Capability::Delegate);
            }

            // Build the child token (signed by daemon owner key)
            let builder = TokenBuilder::new(auth.owner_key.clone())
                .delegated_from(parent_token)
                .with_capabilities(child_caps)
                .with_lifetime(lifetime)
                .with_random_nonce();

            match builder.build() {
                Ok(child_token) => {
                    let b64 = match child_token.to_base64() {
                        Ok(b) => b,
                        Err(e) => return format!("Failed to encode child token: {e}"),
                    };

                    let cap_names: Vec<&str> = child_token.capabilities.iter().map(|c| match c {
                        Capability::Prompt => "Prompt",
                        Capability::ToolUse { .. } => "ToolUse",
                        Capability::ShellExecute { .. } => "ShellExecute",
                        Capability::FileAccess { .. } => "FileAccess",
                        Capability::BotCommand { .. } => "BotCommand",
                        Capability::SessionManage => "SessionManage",
                        Capability::ModelSwitch => "ModelSwitch",
                        Capability::Delegate => "Delegate",
                    }).collect();

                    let expires = chrono::DateTime::from_timestamp(child_token.expires_at as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    format!(
                        "**Child token created** ✓\n\n\
                         • Capabilities: {}\n\
                         • Expires: {}\n\
                         • Depth: {}\n\n\
                         ```\n{}\n```\n\n\
                         Share this with the recipient. They register it with `!token <token>`.",
                        cap_names.join(", "),
                        expires,
                        child_token.delegation_depth,
                        b64,
                    )
                }
                Err(e) => format!("**Delegation failed:** {e}"),
            }
        }
        _ => {
            // Unknown ! command — pass to agent as a normal prompt
            run_matrix_prompt(Arc::clone(&store), key.clone(), body.to_string(), None).await
        }
    }
}

/// Parse duration strings like "30m", "1h", "7d", "1y" into `std::time::Duration`.
pub(crate) fn parse_delegate_duration(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();
    let (num_str, unit) = if s.ends_with('m') {
        (&s[..s.len() - 1], 'm')
    } else if s.ends_with('h') {
        (&s[..s.len() - 1], 'h')
    } else if s.ends_with('d') {
        (&s[..s.len() - 1], 'd')
    } else if s.ends_with('y') {
        (&s[..s.len() - 1], 'y')
    } else {
        return None;
    };
    let num: u64 = num_str.parse().ok()?;
    let secs = match unit {
        'm' => num * 60,
        'h' => num * 3600,
        'd' => num * 86400,
        'y' => num * 86400 * 365,
        _ => return None,
    };
    Some(std::time::Duration::from_secs(secs))
}

// ── Sendfile tag extraction ──────────────────────────────────────────────

/// A file the agent wants to send back to the user.
pub(crate) struct SendfileTag {
    /// Absolute path to the file
    pub(crate) path: String,
}

/// Extract `<sendfile>/path</sendfile>` tags from response text.
/// Returns the cleaned text (tags stripped) and a list of file paths.
pub(crate) fn extract_sendfile_tags(text: &str) -> (String, Vec<SendfileTag>) {
    let mut cleaned = String::with_capacity(text.len());
    let mut tags = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find("<sendfile>") {
        // Copy text before the tag
        cleaned.push_str(&remaining[..start]);

        let after_open = &remaining[start + "<sendfile>".len()..];
        if let Some(end) = after_open.find("</sendfile>") {
            let path = after_open[..end].trim().to_string();
            if !path.is_empty() {
                tags.push(SendfileTag { path });
            }
            remaining = &after_open[end + "</sendfile>".len()..];
        } else {
            // Unclosed tag — keep it as-is (prefix already pushed above)
            cleaned.push_str("<sendfile>");
            remaining = after_open;
        }
    }
    cleaned.push_str(remaining);

    (cleaned, tags)
}

/// Guess MIME type from file extension.
pub(crate) fn guess_mime(path: &std::path::Path) -> mime::Mime {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext.to_lowercase().as_str() {
        "png" => mime::IMAGE_PNG,
        "jpg" | "jpeg" => mime::IMAGE_JPEG,
        "gif" => mime::IMAGE_GIF,
        "webp" => "image/webp".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "svg" => mime::IMAGE_SVG,
        "mp4" => "video/mp4".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "webm" => "video/webm".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "mp3" => "audio/mpeg".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "ogg" => "audio/ogg".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "wav" => "audio/wav".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "pdf" => mime::APPLICATION_PDF,
        "txt" | "md" | "rs" | "py" | "js" | "ts" | "toml" | "yaml" | "yml" | "json" => mime::TEXT_PLAIN,
        _ => mime::APPLICATION_OCTET_STREAM,
    }
}

/// Check whether a path is safe to send over Matrix.
///
/// Blocks known sensitive directories and files to prevent the agent from
/// accidentally exfiltrating credentials, keys, or system secrets.
pub(crate) fn is_sendfile_path_allowed(path: &std::path::Path) -> std::result::Result<(), String> {
    // Canonicalize to resolve symlinks and ../ tricks
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("cannot resolve path: {e}"))?;
    let s = canonical.to_string_lossy();

    // Blocked directory prefixes (home-relative)
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        let blocked_dirs = [
            ".ssh",
            ".gnupg",
            ".gpg",
            ".aws",
            ".azure",
            ".config/gcloud",
            ".kube",
            ".docker",
            ".npmrc",
            ".pypirc",
            ".netrc",
            ".clankers/matrix.json",
        ];
        for dir in &blocked_dirs {
            let blocked = format!("{}/{}", home_str, dir);
            if s.starts_with(&blocked) {
                return Err(format!("blocked: path inside ~/{dir}"));
            }
        }
    }

    // Blocked system paths
    let blocked_system = [
        "/etc/shadow",
        "/etc/gshadow",
        "/etc/master.passwd",
        "/etc/sudoers",
    ];
    for bp in &blocked_system {
        if s.as_ref() == *bp || s.starts_with(&format!("{bp}.")) {
            return Err(format!("blocked: sensitive system file {bp}"));
        }
    }

    // Block private key files by name pattern
    let filename = canonical
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    let blocked_names = [
        "id_rsa",
        "id_ed25519",
        "id_ecdsa",
        "id_dsa",
        ".env",
        ".env.local",
        ".env.production",
    ];
    for bn in &blocked_names {
        if filename == *bn {
            return Err(format!("blocked: sensitive file name {bn}"));
        }
    }

    Ok(())
}

/// Upload sendfile tags to Matrix and return error annotations for failures.
pub(crate) async fn upload_sendfiles(
    client: &tokio::sync::RwLock<clankers_matrix::MatrixClient>,
    room_id: &clankers_matrix::ruma::OwnedRoomId,
    tags: &[SendfileTag],
) -> Vec<String> {
    let mut errors = Vec::new();

    for tag in tags {
        let path = std::path::Path::new(&tag.path);

        if !path.exists() || !path.is_file() {
            errors.push(format!("(failed to send file {}: file not found)", tag.path));
            continue;
        }

        // Path validation: block sensitive files
        if let Err(reason) = is_sendfile_path_allowed(path) {
            warn!("Sendfile blocked: {} ({})", tag.path, reason);
            errors.push(format!("(refused to send file {}: {})", tag.path, reason));
            continue;
        }

        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                errors.push(format!(
                    "(failed to send file {}: {})",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    e
                ));
                continue;
            }
        };

        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let content_type = guess_mime(path);

        let c = client.read().await;
        if let Err(e) = c.send_file(room_id, &filename, &content_type, data).await {
            errors.push(format!("(failed to send file {}: {})", filename, e));
        } else {
            info!("Uploaded file to Matrix: {}", filename);
        }
    }

    errors
}

/// Run a prompt for a Matrix message and collect the full text response.
///
/// If `capabilities` is Some, the session is created with filtered tools.
/// If None, full access (allowlist user).
pub(crate) async fn run_matrix_prompt(
    store: Arc<RwLock<SessionStore>>,
    key: SessionKey,
    text: String,
    capabilities: Option<&[Capability]>,
) -> String {
    // Get conversation history
    let (mut agent, history) = {
        let mut store = store.write().await;
        let session = store.get_or_create(&key, capabilities);
        session.turn_count += 1;
        let messages = session.agent.messages().to_vec();
        let tools = session.session_tools.clone();
        let provider = Arc::clone(&store.provider);
        let settings = store.settings.clone();
        let model = store.model.clone();
        let system_prompt = store.system_prompt.clone();
        let agent = crate::agent::builder::AgentBuilder::new(provider, settings.clone(), model, system_prompt)
            .with_tools(tools)
            .build();
        (agent, messages)
    };

    agent.seed_messages(history);

    let mut rx = agent.subscribe();

    let collector = tokio::spawn(async move {
        let mut collected = String::new();
        while let Ok(event) = rx.recv().await {
            if let AgentEvent::MessageUpdate {
                delta: ContentDelta::TextDelta { ref text },
                ..
            } = event
            {
                collected.push_str(text);
            }
            if matches!(event, AgentEvent::AgentEnd { .. }) {
                break;
            }
        }
        collected
    });

    let result = agent.prompt(&text).await;
    let collected = collector.await.unwrap_or_default();

    // Save updated messages back
    let messages = agent.messages().to_vec();
    {
        let mut store = store.write().await;
        if let Some(session) = store.sessions.get_mut(&key) {
            session.agent.seed_messages(messages);
        }
    }

    match result {
        Ok(()) => collected,
        Err(e) => format!("Error: {e}"),
    }
}

/// Run a prompt with image content blocks (for vision models).
pub(crate) async fn run_matrix_prompt_with_images(
    store: Arc<RwLock<SessionStore>>,
    key: SessionKey,
    text: String,
    images: Vec<Content>,
    capabilities: Option<&[Capability]>,
) -> String {
    let (mut agent, history) = {
        let mut store = store.write().await;
        let session = store.get_or_create(&key, capabilities);
        session.turn_count += 1;
        let messages = session.agent.messages().to_vec();
        let tools = session.session_tools.clone();
        let provider = Arc::clone(&store.provider);
        let settings = store.settings.clone();
        let model = store.model.clone();
        let system_prompt = store.system_prompt.clone();
        let agent = crate::agent::builder::AgentBuilder::new(provider, settings.clone(), model, system_prompt)
            .with_tools(tools)
            .build();
        (agent, messages)
    };

    agent.seed_messages(history);

    let mut rx = agent.subscribe();

    let collector = tokio::spawn(async move {
        let mut collected = String::new();
        while let Ok(event) = rx.recv().await {
            if let AgentEvent::MessageUpdate {
                delta: ContentDelta::TextDelta { ref text },
                ..
            } = event
            {
                collected.push_str(text);
            }
            if matches!(event, AgentEvent::AgentEnd { .. }) {
                break;
            }
        }
        collected
    });

    let result = agent.prompt_with_images(&text, images).await;
    let collected = collector.await.unwrap_or_default();

    let messages = agent.messages().to_vec();
    {
        let mut store = store.write().await;
        if let Some(session) = store.sessions.get_mut(&key) {
            session.agent.seed_messages(messages);
        }
    }

    match result {
        Ok(()) => collected,
        Err(e) => format!("Error: {e}"),
    }
}

// ── Proactive agent: heartbeat + trigger pipes ──────────────────────────────

/// Check whether a response signals "nothing to report".
pub(crate) fn is_heartbeat_ok(response: &str) -> bool {
    let upper = response.to_uppercase();
    upper.contains("HEARTBEAT_OK") || upper.contains("HEARTBEAT OK")
}

/// Run a prompt against a session without updating `last_active`.
/// Used for heartbeat and trigger prompts — these shouldn't prevent
/// idle reaping.
pub(crate) async fn run_proactive_prompt(
    store: Arc<RwLock<SessionStore>>,
    key: SessionKey,
    text: String,
) -> String {
    // Serialize via prompt lock
    let prompt_lock = {
        let mut store = store.write().await;
        store.prompt_lock(&key)
    };
    let _prompt_guard = prompt_lock.lock().await;

    let (mut agent, history) = {
        let mut store = store.write().await;
        let session = match store.sessions.get_mut(&key) {
            Some(s) => s,
            None => return String::new(), // session gone
        };
        // Deliberately do NOT update last_active or turn_count
        let messages = session.agent.messages().to_vec();
        let tools = session.session_tools.clone();
        let provider = Arc::clone(&store.provider);
        let settings = store.settings.clone();
        let model = store.model.clone();
        let system_prompt = store.system_prompt.clone();
        let agent = crate::agent::builder::AgentBuilder::new(provider, settings.clone(), model, system_prompt)
            .with_tools(tools)
            .build();
        (agent, messages)
    };

    agent.seed_messages(history);

    let mut rx = agent.subscribe();

    let collector = tokio::spawn(async move {
        let mut collected = String::new();
        while let Ok(event) = rx.recv().await {
            if let AgentEvent::MessageUpdate {
                delta: ContentDelta::TextDelta { ref text },
                ..
            } = event
            {
                collected.push_str(text);
            }
            if matches!(event, AgentEvent::AgentEnd { .. }) {
                break;
            }
        }
        collected
    });

    let result = agent.prompt(&text).await;
    let collected = collector.await.unwrap_or_default();

    // Save updated messages back
    let messages = agent.messages().to_vec();
    {
        let mut store = store.write().await;
        if let Some(session) = store.sessions.get_mut(&key) {
            session.agent.seed_messages(messages);
        }
    }

    match result {
        Ok(()) => collected,
        Err(e) => format!("Error: {e}"),
    }
}

/// Ensure a trigger pipe reader is running for a Matrix session.
/// No-op if the session already has one or the key is not Matrix.
pub(crate) async fn ensure_trigger_pipe(
    store: Arc<RwLock<SessionStore>>,
    key: &SessionKey,
    matrix_client: Arc<tokio::sync::RwLock<clankers_matrix::MatrixClient>>,
) {
    if key.matrix_room_id().is_none() {
        return;
    }
    let needs_spawn = {
        let store = store.read().await;
        match store.sessions.get(key) {
            Some(s) => s.trigger_cancel.is_none(),
            None => false,
        }
    };

    if !needs_spawn {
        return;
    }

    let session_dir = {
        let store = store.read().await;
        match store.sessions.get(key) {
            Some(s) => s.session_dir.clone(),
            None => return,
        }
    };

    let cancel = spawn_trigger_reader(
        &session_dir,
        key.clone(),
        Arc::clone(&store),
        matrix_client,
    );

    if let Some(cancel) = cancel {
        let mut store = store.write().await;
        if let Some(session) = store.sessions.get_mut(key) {
            session.trigger_cancel = Some(cancel);
        }
    }
}

/// Run the per-session heartbeat scheduler.
///
/// Iterates all active Matrix sessions, checks for HEARTBEAT.md,
/// and prompts the agent if the file is non-empty. Responses
/// containing "HEARTBEAT_OK" are suppressed.
pub(crate) async fn run_session_heartbeat(
    store: Arc<RwLock<SessionStore>>,
    matrix_client: Arc<tokio::sync::RwLock<clankers_matrix::MatrixClient>>,
    interval: std::time::Duration,
    heartbeat_prompt: String,
    cancel: CancellationToken,
) {
    let mut ticker = tokio::time::interval(interval);
    ticker.tick().await; // skip the immediate first tick

    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            () = cancel.cancelled() => break,
        }

        // Snapshot all Matrix sessions that have a HEARTBEAT.md
        let targets: Vec<(SessionKey, PathBuf, String)> = {
            let store = store.read().await;
            store
                .sessions
                .iter()
                .filter_map(|(key, session)| {
                    let room_id = key.matrix_room_id()?.to_string();
                    let hb_path = session.session_dir.join("HEARTBEAT.md");
                    Some((key.clone(), hb_path, room_id))
                })
                .collect()
        };

        for (key, hb_path, room_id) in targets {
            // Read heartbeat file
            let contents = match tokio::fs::read_to_string(&hb_path).await {
                Ok(c) if !c.trim().is_empty() => c,
                _ => continue, // missing or empty — skip
            };

            info!("[{}] heartbeat: found {} bytes in HEARTBEAT.md", key, contents.len());

            let prompt = format!("{}\n\n---\n\n{}", heartbeat_prompt, contents);

            // Start typing
            let room_id_parsed = match clankers_matrix::ruma::RoomId::parse(&room_id) {
                Ok(rid) => rid.clone(),
                Err(_) => continue,
            };
            {
                let c = matrix_client.read().await;
                let _ = c.set_typing(&room_id_parsed, true).await;
            }

            // Prompt the agent (without updating last_active)
            let response = run_proactive_prompt(
                Arc::clone(&store),
                key.clone(),
                prompt,
            )
            .await;

            // Stop typing
            {
                let c = matrix_client.read().await;
                let _ = c.set_typing(&room_id_parsed, false).await;
            }

            // Suppress HEARTBEAT_OK responses
            if is_heartbeat_ok(&response) {
                info!("[{}] heartbeat: OK (suppressed)", key);
                continue;
            }

            if response.trim().is_empty() {
                info!("[{}] heartbeat: empty response (suppressed)", key);
                continue;
            }

            // Send response to Matrix
            let c = matrix_client.read().await;
            if let Err(e) = c.send_markdown(&room_id_parsed, &response).await {
                error!("[{}] heartbeat send failed: {e}", key);
            } else {
                info!("[{}] heartbeat: sent response ({} bytes)", key, response.len());
            }
        }
    }
}

/// Create a named pipe (FIFO) at the given path.
/// Returns Ok(()) if created or already exists, Err on failure.
pub(crate) fn create_fifo(path: &std::path::Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }

    let c_path = std::ffi::CString::new(path.to_string_lossy().as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    // Mode 0o660: owner+group read/write
    let result = unsafe { libc::mkfifo(c_path.as_ptr(), 0o660) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Spawn a trigger pipe reader task for a session.
///
/// Creates a FIFO at `{session_dir}/trigger.pipe` and reads lines from it.
/// Each line becomes a prompt to the agent; responses go to the Matrix room.
/// Returns the cancellation token used to stop the reader.
pub(crate) fn spawn_trigger_reader(
    session_dir: &std::path::Path,
    key: SessionKey,
    store: Arc<RwLock<SessionStore>>,
    matrix_client: Arc<tokio::sync::RwLock<clankers_matrix::MatrixClient>>,
) -> Option<CancellationToken> {
    let room_id = key.matrix_room_id()?.to_string();
    let pipe_path = session_dir.join("trigger.pipe");

    if let Err(e) = create_fifo(&pipe_path) {
        error!("[{}] failed to create trigger pipe {}: {e}", key, pipe_path.display());
        return None;
    }

    info!("[{}] trigger pipe: {}", key, pipe_path.display());

    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;

        loop {
            // Open the FIFO — this blocks until a writer opens the other end.
            // When the writer closes, we get EOF and re-open.
            let file = tokio::select! {
                f = tokio::fs::File::open(&pipe_path) => {
                    match f {
                        Ok(f) => f,
                        Err(e) => {
                            warn!("[{}] trigger pipe open failed: {e}", key);
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            continue;
                        }
                    }
                }
                () = cancel_clone.cancelled() => break,
            };

            let reader = tokio::io::BufReader::new(file);
            let mut lines = reader.lines();

            loop {
                let line = tokio::select! {
                    l = lines.next_line() => l,
                    () = cancel_clone.cancelled() => break,
                };

                match line {
                    Ok(Some(text)) if !text.trim().is_empty() => {
                        info!("[{}] trigger: {}", key, &text[..80.min(text.len())]);

                        let room_id_parsed = match clankers_matrix::ruma::RoomId::parse(&room_id) {
                            Ok(rid) => rid.clone(),
                            Err(_) => continue,
                        };

                        // Typing indicator
                        {
                            let c = matrix_client.read().await;
                            let _ = c.set_typing(&room_id_parsed, true).await;
                        }

                        let response = run_proactive_prompt(
                            Arc::clone(&store),
                            key.clone(),
                            text,
                        )
                        .await;

                        {
                            let c = matrix_client.read().await;
                            let _ = c.set_typing(&room_id_parsed, false).await;
                        }

                        if is_heartbeat_ok(&response) || response.trim().is_empty() {
                            info!("[{}] trigger: suppressed (ok/empty)", key);
                            continue;
                        }

                        let c = matrix_client.read().await;
                        if let Err(e) = c.send_markdown(&room_id_parsed, &response).await {
                            error!("[{}] trigger send failed: {e}", key);
                        }
                    }
                    Ok(Some(_)) => {} // empty line, skip
                    Ok(None) => break, // EOF — writer closed, re-open
                    Err(e) => {
                        warn!("[{}] trigger pipe read error: {e}", key);
                        break;
                    }
                }
            }
        }
    });

    Some(cancel)
}
