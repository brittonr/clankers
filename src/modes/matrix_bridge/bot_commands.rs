//! Matrix bot commands (!help, !status, !token, etc.).

use std::sync::Arc;

use clanker_actor::ProcessRegistry;
use clankers_auth::Capability;
use clankers_auth::CapabilityToken;
use clankers_auth::TokenBuilder;
use clankers_controller::transport::DaemonState;
use clankers_protocol::SessionCommand;
use clankers_protocol::SessionKey;
use tokio::sync::Mutex;

use super::prompt::run_matrix_prompt;
use crate::modes::daemon::session_store::AuthLayer;
use crate::modes::daemon::socket_bridge::SessionFactory;

/// Handle a `!command` from a Matrix user. Returns the response text.
pub(crate) async fn handle_bot_command(
    body: &str,
    key: &SessionKey,
    state: Arc<Mutex<DaemonState>>,
    registry: ProcessRegistry,
    factory: Arc<SessionFactory>,
    auth: Option<Arc<AuthLayer>>,
) -> String {
    let parts: Vec<&str> = body.trim().splitn(2, char::is_whitespace).collect();
    let command = parts[0].to_lowercase();
    let args = parts.get(1).unwrap_or(&"").trim();

    match command.as_str() {
        "!help" => "**Available commands:**\n\
             • `!help` — Show this message\n\
             • `!status` — Session info (model, turns, uptime)\n\
             • `!restart` — Clear session history and start fresh\n\
             • `!compact` — Trigger context compaction\n\
             • `!model <name>` — Switch model for this session\n\
             • `!skills` — List loaded skills\n\
             • `!token <base64>` — Register an access token\n\
             • `!delegate [opts]` — Create a child token from yours"
            .to_string(),
        "!status" => {
            let st = state.lock().await;
            if let Some(handle) = st.session_by_key(key) {
                format!(
                    "**Session status:**\n\
                     • Model: `{}`\n\
                     • Turns: {}\n\
                     • Session ID: `{}`\n\
                     • Last active: {}",
                    handle.model,
                    handle.turn_count,
                    handle.session_id,
                    handle.last_active,
                )
            } else {
                "No active session. Send a message to start one.".to_string()
            }
        }
        "!restart" => {
            // Kill the existing session and remove from state
            let mut st = state.lock().await;
            if let Some(session_id) = st.key_index.get(key).cloned() {
                if let Some(handle) = st.sessions.get(&session_id) {
                    let _ = handle.cmd_tx.send(SessionCommand::Disconnect);
                }
                st.remove_session(&session_id);
            }
            "Session cleared. Next message starts a fresh conversation.".to_string()
        }
        "!compact" => {
            run_matrix_prompt(
                state,
                registry,
                factory,
                key.clone(),
                "/compact".to_string(),
            )
            .await;
            "Context compacted.".to_string()
        }
        "!model" => {
            if args.is_empty() {
                let st = state.lock().await;
                let model = st
                    .session_by_key(key)
                    .map(|h| h.model.clone())
                    .unwrap_or_else(|| factory.default_model.clone());
                format!("Current model: `{}`. Usage: `!model <name>`", model)
            } else {
                // Switch model on the session controller
                let st = state.lock().await;
                if let Some(handle) = st.session_by_key(key) {
                    let old_model = handle.model.clone();
                    let _ = handle.cmd_tx.send(SessionCommand::SetModel {
                        model: args.to_string(),
                    });
                    format!("Model switched: `{}` → `{}`", old_model, args)
                } else {
                    "No active session. Send a message first, then switch model.".to_string()
                }
            }
        }
        "!skills" => {
            let tool_names: Vec<String> = factory
                .tools
                .iter()
                .map(|t| t.definition().name.clone())
                .collect();
            if tool_names.is_empty() {
                "No tools loaded.".to_string()
            } else {
                format!(
                    "**Loaded tools ({}):**\n{}",
                    tool_names.len(),
                    tool_names.iter().map(|n| format!("• `{}`", n)).collect::<Vec<_>>().join("\n")
                )
            }
        }
        "!token" => handle_token_command(args, key, &state, &auth).await,
        "!delegate" => handle_delegate_command(args, key, &auth),
        _ => {
            // Unknown ! command — pass to agent as a normal prompt
            run_matrix_prompt(state, registry, factory, key.clone(), body.to_string()).await
        }
    }
}

async fn handle_token_command(
    args: &str,
    key: &SessionKey,
    state: &Arc<Mutex<DaemonState>>,
    auth: &Option<Arc<AuthLayer>>,
) -> String {
    if args.is_empty() {
        return "Usage: `!token <base64-encoded-token>`\n\n\
                Register an access token to get daemon capabilities.\n\
                Get a token from the daemon owner: `clankers token create`"
            .to_string();
    }

    let Some(auth) = auth else {
        return "Token auth is not enabled on this daemon.".to_string();
    };

    let token = match CapabilityToken::from_base64(args) {
        Ok(t) => t,
        Err(e) => return format!("Invalid token: {e}"),
    };

    match auth.verify_token(&token) {
        Ok(caps) => {
            let user_id = match key {
                SessionKey::Matrix { user_id, .. } => user_id.clone(),
                SessionKey::Iroh(id) => id.clone(),
            };

            auth.store_token(&user_id, &token);

            // Kill existing session so the next message picks up new capabilities
            {
                let mut st = state.lock().await;
                if let Some(session_id) = st.key_index.get(key).cloned() {
                    if let Some(handle) = st.sessions.get(&session_id) {
                        let _ = handle.cmd_tx.send(SessionCommand::Disconnect);
                    }
                    st.remove_session(&session_id);
                }
            }

            let cap_names: Vec<&str> = caps
                .iter()
                .map(|c| match c {
                    Capability::Prompt => "Prompt",
                    Capability::ToolUse { .. } => "ToolUse",
                    Capability::ShellExecute { .. } => "ShellExecute",
                    Capability::FileAccess { .. } => "FileAccess",
                    Capability::BotCommand { .. } => "BotCommand",
                    Capability::SessionManage => "SessionManage",
                    Capability::ModelSwitch => "ModelSwitch",
                    Capability::Delegate => "Delegate",
                })
                .collect();

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

fn handle_delegate_command(
    args: &str,
    key: &SessionKey,
    auth: &Option<Arc<AuthLayer>>,
) -> String {
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

    let Some(auth) = auth else {
        return "Token auth is not enabled on this daemon.".to_string();
    };

    let user_id = match key {
        SessionKey::Matrix { user_id, .. } => user_id.clone(),
        SessionKey::Iroh(id) => id.clone(),
    };

    let parent_token = match auth.lookup_token(&user_id) {
        Some(t) => t,
        None => return "You don't have a registered token. Use `!token <base64>` first.".to_string(),
    };

    if let Err(e) = auth.verify_token(&parent_token) {
        return format!("Your token is invalid: {e}");
    }

    if !parent_token.capabilities.contains(&Capability::Delegate) {
        return "Your token does not have the Delegate capability.".to_string();
    }

    // Parse flags
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

    let lifetime = match expire_str {
        Some(s) => match parse_delegate_duration(s) {
            Some(d) => d,
            None => return format!("Invalid duration: `{s}`. Use e.g. `30m`, `1h`, `7d`, `1y`."),
        },
        None => std::time::Duration::from_secs(3600),
    };

    let now = clankers_auth::utils::current_time_secs();
    let parent_remaining = parent_token.expires_at.saturating_sub(now);
    let lifetime = lifetime.min(std::time::Duration::from_secs(parent_remaining));

    let mut child_caps = vec![Capability::Prompt];
    if let Some(pattern) = tools_pattern {
        child_caps.push(Capability::ToolUse { tool_pattern: pattern });
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

            let cap_names: Vec<&str> = child_token
                .capabilities
                .iter()
                .map(|c| match c {
                    Capability::Prompt => "Prompt",
                    Capability::ToolUse { .. } => "ToolUse",
                    Capability::ShellExecute { .. } => "ShellExecute",
                    Capability::FileAccess { .. } => "FileAccess",
                    Capability::BotCommand { .. } => "BotCommand",
                    Capability::SessionManage => "SessionManage",
                    Capability::ModelSwitch => "ModelSwitch",
                    Capability::Delegate => "Delegate",
                })
                .collect();

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

/// Parse duration strings like "30m", "1h", "7d", "1y".
pub(crate) fn parse_delegate_duration(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();
    let (num_str, unit) = if let Some(stripped) = s.strip_suffix('m') {
        (stripped, 'm')
    } else if let Some(stripped) = s.strip_suffix('h') {
        (stripped, 'h')
    } else if let Some(stripped) = s.strip_suffix('d') {
        (stripped, 'd')
    } else if let Some(stripped) = s.strip_suffix('y') {
        (stripped, 'y')
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
