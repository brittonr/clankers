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
pub(crate) async fn handle_bot_command(
    body: &str,
    key: &SessionKey,
    state: Arc<Mutex<DaemonState>>,
    registry: ProcessRegistry,
    factory: Arc<SessionFactory>,
    auth: Option<Arc<AuthLayer>>,
) -> String {
    let (command, args) = parse_bot_command(body);

    match command.as_str() {
        "!help" => help_text(),
        "!status" => handle_status_command(key, &state).await,
        "!restart" => handle_restart_command(key, &state).await,
        "!compact" => handle_compact_command(key, state, registry, factory).await,
        "!model" => handle_model_command(args, key, &state, &factory).await,
        "!skills" => handle_skills_command(&factory),
        "!token" => handle_token_command(args, key, &state, &auth).await,
        "!delegate" => handle_delegate_command(args, key, &auth),
        _ => run_matrix_prompt(state, registry, factory, key.clone(), body.to_string(), None, None).await,
    }
}

fn parse_bot_command(body: &str) -> (String, &str) {
    let trimmed = body.trim();
    if let Some((command, args)) = trimmed.split_once(char::is_whitespace) {
        return (command.to_lowercase(), args.trim());
    }

    (trimmed.to_lowercase(), "")
}

fn help_text() -> String {
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

async fn handle_status_command(key: &SessionKey, state: &Arc<Mutex<DaemonState>>) -> String {
    let st = state.lock().await;
    if let Some(handle) = st.session_by_key(key) {
        return format!(
            "**Session status:**\n\
             • Model: `{}`\n\
             • Turns: {}\n\
             • Session ID: `{}`\n\
             • Last active: {}",
            handle.model, handle.turn_count, handle.session_id, handle.last_active,
        );
    }

    "No active session. Send a message to start one.".to_string()
}

async fn handle_restart_command(key: &SessionKey, state: &Arc<Mutex<DaemonState>>) -> String {
    let mut st = state.lock().await;
    remove_session_for_key(key, &mut st);
    "Session cleared. Next message starts a fresh conversation.".to_string()
}

async fn handle_compact_command(
    key: &SessionKey,
    state: Arc<Mutex<DaemonState>>,
    registry: ProcessRegistry,
    factory: Arc<SessionFactory>,
) -> String {
    run_matrix_prompt(state, registry, factory, key.clone(), "/compact".to_string(), None, None).await;
    "Context compacted.".to_string()
}

async fn handle_model_command(
    args: &str,
    key: &SessionKey,
    state: &Arc<Mutex<DaemonState>>,
    factory: &Arc<SessionFactory>,
) -> String {
    if args.is_empty() {
        return current_model_message(key, state, factory).await;
    }

    switch_model_message(args, key, state).await
}

async fn current_model_message(
    key: &SessionKey,
    state: &Arc<Mutex<DaemonState>>,
    factory: &Arc<SessionFactory>,
) -> String {
    let st = state.lock().await;
    let model = st.session_by_key(key).map(|h| h.model.clone()).unwrap_or_else(|| factory.default_model.clone());
    format!("Current model: `{}`. Usage: `!model <name>`", model)
}

async fn switch_model_message(args: &str, key: &SessionKey, state: &Arc<Mutex<DaemonState>>) -> String {
    let st = state.lock().await;
    let Some(handle) = st.session_by_key(key) else {
        return "No active session. Send a message first, then switch model.".to_string();
    };

    let old_model = handle.model.clone();
    if let Some(ref tx) = handle.cmd_tx {
        tx.send(SessionCommand::SetModel {
            model: args.to_string(),
        })
        .ok();
    }
    format!("Model switched: `{}` → `{}`", old_model, args)
}

fn handle_skills_command(factory: &SessionFactory) -> String {
    let tool_names: Vec<String> = factory.tools.iter().map(|tool| tool.definition().name.clone()).collect();
    if tool_names.is_empty() {
        return "No tools loaded.".to_string();
    }

    let tool_list = tool_names.iter().map(|name| format!("• `{}`", name)).collect::<Vec<_>>().join("\n");
    format!("**Loaded tools ({}):**\n{}", tool_names.len(), tool_list)
}

fn remove_session_for_key(key: &SessionKey, state: &mut DaemonState) {
    if let Some(session_id) = state.key_index.get(key).cloned() {
        if let Some(handle) = state.sessions.get(&session_id)
            && let Some(ref tx) = handle.cmd_tx
        {
            tx.send(SessionCommand::Disconnect).ok();
        }
        state.remove_session(&session_id);
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
                remove_session_for_key(key, &mut st);
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
