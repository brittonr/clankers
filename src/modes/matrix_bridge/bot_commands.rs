//! Matrix bot commands (!help, !status, !token, etc.).

use std::sync::Arc;

use clanker_actor::ProcessRegistry;
use clankers_controller::transport::DaemonState;
use clankers_protocol::SessionCommand;
use clankers_protocol::SessionKey;
use tokio::sync::Mutex;

use super::prompt::run_matrix_prompt;
use crate::modes::daemon::session_store::AuthLayer;
use crate::modes::daemon::session_store::session_prompt_admission_request;
use crate::modes::daemon::socket_bridge::SessionFactory;

/// Handle a `!command` from a Matrix user. Returns the response text.
#[cfg_attr(dylint_lib = "tigerstyle", allow(function_length, reason = "command dispatch logic"))]
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
                    handle.model, handle.turn_count, handle.session_id, handle.last_active,
                )
            } else {
                "No active session. Send a message to start one.".to_string()
            }
        }
        "!restart" => {
            // Kill the existing session and remove from state
            let mut st = state.lock().await;
            if let Some(session_id) = st.key_index.get(key).cloned() {
                if let Some(handle) = st.sessions.get(&session_id)
                    && let Some(ref tx) = handle.cmd_tx
                {
                    tx.send(SessionCommand::Disconnect).ok();
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
                None, // session already exists with capabilities from initial auth
                None,
            )
            .await;
            "Context compacted.".to_string()
        }
        "!model" => {
            if args.is_empty() {
                let st = state.lock().await;
                let model =
                    st.session_by_key(key).map(|h| h.model.clone()).unwrap_or_else(|| factory.default_model.clone());
                format!("Current model: `{}`. Usage: `!model <name>`", model)
            } else {
                // Switch model on the session controller
                let st = state.lock().await;
                if let Some(handle) = st.session_by_key(key) {
                    let old_model = handle.model.clone();
                    if let Some(ref tx) = handle.cmd_tx {
                        tx.send(SessionCommand::SetModel {
                            model: args.to_string(),
                        })
                        .ok();
                    }
                    format!("Model switched: `{}` → `{}`", old_model, args)
                } else {
                    "No active session. Send a message first, then switch model.".to_string()
                }
            }
        }
        "!skills" => {
            let tool_names: Vec<String> = factory.tools.iter().map(|t| t.definition().name.clone()).collect();
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
            run_matrix_prompt(state, registry, factory, key.clone(), body.to_string(), None, None).await
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

    let user_id = match key {
        SessionKey::Matrix { user_id, .. } => user_id.clone(),
        SessionKey::Iroh(id) => id.clone(),
    };
    let request = session_prompt_admission_request(&user_id);

    match auth.verify_credential_base64(args, &request) {
        Ok((cred, receipt)) => {
            auth.store_credential(&user_id, &cred);

            // Kill existing session so the next message picks up new capabilities.
            {
                let mut st = state.lock().await;
                if let Some(session_id) = st.key_index.get(key).cloned() {
                    if let Some(handle) = st.sessions.get(&session_id)
                        && let Some(ref tx) = handle.cmd_tx
                    {
                        tx.send(SessionCommand::Disconnect).ok();
                    }
                    st.remove_session(&session_id);
                }
            }

            format!(
                "**Public UCAN credential accepted** ✓\n\n\
                 • Token reference: `{}`\n\
                 • Audience: `{}`\n\
                 • Policy: `{}`\n\
                 • Replay: `{}`\n\n\
                 Your session has been restarted with the new public UCAN/Basalt gate.",
                receipt.token_reference, receipt.audience, receipt.policy_hash, receipt.replay_status,
            )
        }
        Err(e) => format!("**Credential rejected:** {e}"),
    }
}

fn handle_delegate_command(_args: &str, _key: &SessionKey, _auth: &Option<Arc<AuthLayer>>) -> String {
    "Public UCAN delegation is explicit now. Ask the daemon owner to issue a delegated public UCAN envelope for your recipient DID, then register it with `!token <base64>`.".to_string()
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
    } else {
        let stripped = s.strip_suffix('y')?;
        (stripped, 'y')
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
