//! Background agent task — handles AgentCommand variants in a spawned task.
//!
//! Extracted from interactive.rs to isolate the long-running command dispatch
//! loop that processes prompts, login, model switching, etc.

use std::sync::Arc;

use crate::agent::Agent;
use crate::tools::Tool;

use super::interactive::AgentCommand;
use super::interactive::TaskResult;

/// Spawn the background agent task that processes commands.
///
/// The task receives `AgentCommand`s on `cmd_rx`, processes them (potentially
/// streaming responses), and sends `TaskResult`s back on `done_tx`.
///
/// Returns immediately — the actual work happens in the spawned task.
pub(crate) fn spawn_agent_task(
    mut agent: Agent,
    mut cmd_rx: tokio::sync::mpsc::UnboundedReceiver<AgentCommand>,
    done_tx: tokio::sync::mpsc::UnboundedSender<TaskResult>,
    tool_env_for_rebuild: crate::modes::common::ToolEnv,
    plugin_manager: Option<Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
) {
    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                AgentCommand::Prompt(text) => {
                    handle_prompt(&mut agent, &mut cmd_rx, &done_tx, &text, None).await;
                }
                AgentCommand::PromptWithImages { text, images } => {
                    let img_contents: Vec<crate::provider::message::Content> = images
                        .into_iter()
                        .map(|img| crate::provider::message::Content::Image {
                            source: crate::provider::message::ImageSource::Base64 {
                                media_type: img.media_type,
                                data: img.data,
                            },
                        })
                        .collect();
                    handle_prompt(&mut agent, &mut cmd_rx, &done_tx, &text, Some(img_contents)).await;
                }
                AgentCommand::Login {
                    code,
                    state,
                    verifier,
                    account,
                } => {
                    handle_login(&mut agent, &done_tx, &code, &state, &verifier, &account).await;
                }
                AgentCommand::Abort => agent.abort(),
                AgentCommand::ResetCancel => agent.reset_cancel(),
                AgentCommand::SetModel(model) => agent.set_model(model),
                AgentCommand::ClearHistory => agent.clear_messages(),
                AgentCommand::TruncateMessages(n) => agent.truncate_messages(n),
                AgentCommand::SeedMessages(msgs) => agent.seed_messages(msgs),
                AgentCommand::SetThinkingLevel(level) => {
                    let level = agent.set_thinking_level(level);
                    let _ = done_tx.send(TaskResult::ThinkingToggled(thinking_msg(&level), level));
                }
                AgentCommand::CycleThinkingLevel => {
                    let level = agent.cycle_thinking_level();
                    let _ = done_tx.send(TaskResult::ThinkingToggled(thinking_msg(&level), level));
                }
                AgentCommand::SetSystemPrompt(prompt) => {
                    agent.set_system_prompt(prompt);
                }
                AgentCommand::GetSystemPrompt(tx) => {
                    let _ = tx.send(agent.system_prompt().to_string());
                }
                AgentCommand::SwitchAccount(account_name) => {
                    handle_switch_account(&mut agent, &done_tx, &account_name).await;
                }
                AgentCommand::SetDisabledTools(disabled) => {
                    let all_tools = crate::modes::common::build_all_tools_with_env(
                        &tool_env_for_rebuild,
                        plugin_manager.as_ref(),
                    );
                    let filtered: Vec<Arc<dyn Tool>> = all_tools
                        .into_iter()
                        .filter(|t| !disabled.contains(&t.definition().name))
                        .collect();
                    agent = agent.with_tools(filtered);
                }
                AgentCommand::Quit => break,
            }
        }
    });
}

// ── Command handlers ─────────────────────────────────────────────────

/// Handle a prompt (with optional images), supporting mid-stream abort.
async fn handle_prompt(
    agent: &mut Agent,
    cmd_rx: &mut tokio::sync::mpsc::UnboundedReceiver<AgentCommand>,
    done_tx: &tokio::sync::mpsc::UnboundedSender<TaskResult>,
    text: &str,
    images: Option<Vec<crate::provider::message::Content>>,
) {
    agent.reset_cancel();
    let cancel = agent.cancel_token();

    let result = match images {
        Some(img_contents) => {
            let prompt_fut = agent.prompt_with_images(text, img_contents);
            run_prompt_with_abort(prompt_fut, cmd_rx, &cancel).await
        }
        None => {
            let prompt_fut = agent.prompt(text);
            run_prompt_with_abort(prompt_fut, cmd_rx, &cancel).await
        }
    };

    let err = match result {
        Ok(()) => None,
        Err(clankers_agent::AgentError::Cancelled) => None,
        Err(e) => Some(crate::error::Error::from(e)),
    };
    let _ = done_tx.send(TaskResult::PromptDone(err));
}

/// Run a prompt future, listening for Abort commands during streaming.
async fn run_prompt_with_abort<F>(
    prompt_fut: F,
    cmd_rx: &mut tokio::sync::mpsc::UnboundedReceiver<AgentCommand>,
    cancel: &tokio_util::sync::CancellationToken,
) -> std::result::Result<(), clankers_agent::AgentError>
where
    F: std::future::Future<Output = std::result::Result<(), clankers_agent::AgentError>>,
{
    tokio::pin!(prompt_fut);
    loop {
        tokio::select! {
            biased;
            result = &mut prompt_fut => return result,
            Some(cmd) = cmd_rx.recv() => {
                if matches!(cmd, AgentCommand::Abort) {
                    cancel.cancel();
                }
                // Other commands during prompt are dropped;
                // they'll be re-sent after PromptDone.
            }
        }
    }
}

/// Handle OAuth login flow.
async fn handle_login(
    agent: &mut Agent,
    done_tx: &tokio::sync::mpsc::UnboundedSender<TaskResult>,
    code: &str,
    state: &str,
    verifier: &str,
    account: &str,
) {
    use crate::provider::auth::AuthStoreExt;
    let result = clankers_router::oauth::exchange_code(code, state, verifier).await;
    match result {
        Ok(creds) => {
            let paths = crate::config::ClankersPaths::get();
            let mut store = crate::provider::auth::AuthStore::load(&paths.global_auth);
            store.set_credentials(account, creds);
            store.switch_anthropic_account(account);
            match store.save(&paths.global_auth) {
                Ok(()) => {
                    agent.provider().reload_credentials().await;
                    let _ = done_tx.send(TaskResult::LoginDone(Ok(format!(
                        "Authentication successful! Saved as account '{}'.",
                        account
                    ))));
                }
                Err(e) => {
                    let _ = done_tx.send(TaskResult::LoginDone(Err(format!(
                        "Failed to save credentials: {}",
                        e
                    ))));
                }
            }
        }
        Err(e) => {
            let _ = done_tx.send(TaskResult::LoginDone(Err(format!("Login failed: {}", e))));
        }
    }
}

/// Handle account switching.
async fn handle_switch_account(
    agent: &mut Agent,
    done_tx: &tokio::sync::mpsc::UnboundedSender<TaskResult>,
    account_name: &str,
) {
    use crate::provider::auth::AuthStoreExt;
    let paths = crate::config::ClankersPaths::get();
    let mut store = crate::provider::auth::AuthStore::load(&paths.global_auth);
    if store.switch_anthropic_account(account_name) {
        if let Err(e) = store.save(&paths.global_auth) {
            let _ = done_tx.send(TaskResult::AccountSwitched(Err(format!("Failed to save: {}", e))));
        } else {
            agent.provider().reload_credentials().await;
            let _ = done_tx.send(TaskResult::AccountSwitched(Ok(account_name.to_string())));
        }
    } else {
        let _ = done_tx.send(TaskResult::AccountSwitched(Err(format!(
            "No account '{}'",
            account_name
        ))));
    }
}

/// Format a thinking level change into a user-facing message.
fn thinking_msg(level: &crate::provider::ThinkingLevel) -> String {
    if level.is_enabled() {
        format!(
            "Thinking: {} ({} tokens)",
            level.label(),
            level.budget_tokens().unwrap_or(0)
        )
    } else {
        "Thinking: off".to_string()
    }
}
