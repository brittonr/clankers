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
                AgentCommand::RewriteAndPrompt(text) => {
                    let improved = rewrite_prompt(agent.provider(), agent.model(), &text).await;
                    handle_prompt(&mut agent, &mut cmd_rx, &done_tx, &improved, None).await;
                }
                AgentCommand::RewriteAndPromptWithImages { text, images } => {
                    let improved = rewrite_prompt(agent.provider(), agent.model(), &text).await;
                    let img_contents: Vec<crate::provider::message::Content> = images
                        .into_iter()
                        .map(|img| crate::provider::message::Content::Image {
                            source: crate::provider::message::ImageSource::Base64 {
                                media_type: img.media_type,
                                data: img.data,
                            },
                        })
                        .collect();
                    handle_prompt(&mut agent, &mut cmd_rx, &done_tx, &improved, Some(img_contents)).await;
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

/// Rewrite/improve a user prompt using a one-off LLM call.
///
/// Makes a lightweight completion request with a meta-prompt that asks
/// the model to improve the user's prompt for clarity and specificity.
/// Falls back to the original text if the rewrite call fails.
pub(crate) async fn rewrite_prompt(
    provider: &std::sync::Arc<dyn crate::provider::Provider>,
    model: &str,
    original: &str,
) -> String {
    use crate::provider::message::{AgentMessage, Content, MessageId, UserMessage};
    use crate::provider::streaming::StreamEvent;
    use crate::provider::CompletionRequest;

    let system = "You are a prompt engineer. Your job is to rewrite the user's prompt \
        to be clearer, more specific, and more effective for an AI coding assistant. \
        Preserve the original intent completely. Output ONLY the improved prompt text — \
        no commentary, no explanation, no wrapping quotes.";

    let user_msg = AgentMessage::User(UserMessage {
        id: MessageId::generate(),
        content: vec![Content::Text {
            text: original.to_string(),
        }],
        timestamp: chrono::Utc::now(),
    });

    let request = CompletionRequest {
        model: model.to_string(),
        messages: vec![user_msg],
        system_prompt: Some(system.to_string()),
        max_tokens: Some(4096),
        temperature: Some(0.3),
        tools: vec![],
        thinking: None,
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamEvent>(64);

    let complete_handle = {
        let provider = provider.clone();
        tokio::spawn(async move {
            provider.complete(request, tx).await
        })
    };

    let mut result = String::new();
    while let Some(event) = rx.recv().await {
        if let StreamEvent::ContentBlockDelta {
            delta: crate::provider::streaming::ContentDelta::TextDelta { text },
            ..
        } = event
        {
            result.push_str(&text);
        }
    }

    // Wait for the completion to finish
    match complete_handle.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            tracing::warn!("Prompt rewrite failed: {}", e);
            return original.to_string();
        }
        Err(e) => {
            tracing::warn!("Prompt rewrite task panicked: {}", e);
            return original.to_string();
        }
    }

    let improved = result.trim().to_string();
    if improved.is_empty() {
        original.to_string()
    } else {
        improved
    }
}

/// Format a thinking level change into a user-facing message.
#[allow(clippy::needless_pass_by_value)]
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use tokio::sync::mpsc;

    use crate::provider::CompletionRequest;
    use crate::provider::Model;
    use crate::provider::Provider;
    use crate::provider::streaming::ContentDelta;
    use crate::provider::streaming::StreamEvent;

    /// Mock provider that streams back a fixed response.
    struct MockRewriteProvider {
        response: String,
    }

    #[async_trait]
    impl Provider for MockRewriteProvider {
        async fn complete(
            &self,
            _request: CompletionRequest,
            tx: mpsc::Sender<StreamEvent>,
        ) -> crate::provider::error::Result<()> {
            let _ = tx
                .send(StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: ContentDelta::TextDelta {
                        text: self.response.clone(),
                    },
                })
                .await;
            let _ = tx.send(StreamEvent::MessageStop).await;
            Ok(())
        }

        fn models(&self) -> &[Model] {
            &[]
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    /// Mock provider that always returns an error.
    struct FailingProvider;

    #[async_trait]
    impl Provider for FailingProvider {
        async fn complete(
            &self,
            _request: CompletionRequest,
            _tx: mpsc::Sender<StreamEvent>,
        ) -> crate::provider::error::Result<()> {
            Err(crate::provider::error::provider_err("intentional test failure"))
        }

        fn models(&self) -> &[Model] {
            &[]
        }

        fn name(&self) -> &str {
            "failing"
        }
    }

    /// Mock provider that streams an empty response.
    struct EmptyProvider;

    #[async_trait]
    impl Provider for EmptyProvider {
        async fn complete(
            &self,
            _request: CompletionRequest,
            tx: mpsc::Sender<StreamEvent>,
        ) -> crate::provider::error::Result<()> {
            let _ = tx.send(StreamEvent::MessageStop).await;
            Ok(())
        }

        fn models(&self) -> &[Model] {
            &[]
        }

        fn name(&self) -> &str {
            "empty"
        }
    }

    #[tokio::test]
    async fn rewrite_prompt_returns_improved_text() {
        let provider: Arc<dyn Provider> = Arc::new(MockRewriteProvider {
            response: "Improved version of the prompt".to_string(),
        });
        let result = super::rewrite_prompt(&provider, "test-model", "fix the bug").await;
        assert_eq!(result, "Improved version of the prompt");
    }

    #[tokio::test]
    async fn rewrite_prompt_falls_back_on_error() {
        let provider: Arc<dyn Provider> = Arc::new(FailingProvider);
        let result = super::rewrite_prompt(&provider, "test-model", "fix the bug").await;
        assert_eq!(result, "fix the bug");
    }

    #[tokio::test]
    async fn rewrite_prompt_falls_back_on_empty_response() {
        let provider: Arc<dyn Provider> = Arc::new(EmptyProvider);
        let result = super::rewrite_prompt(&provider, "test-model", "fix the bug").await;
        assert_eq!(result, "fix the bug");
    }

    #[tokio::test]
    async fn rewrite_prompt_strips_whitespace() {
        let provider: Arc<dyn Provider> = Arc::new(MockRewriteProvider {
            response: "  improved prompt  \n".to_string(),
        });
        let result = super::rewrite_prompt(&provider, "test-model", "original").await;
        assert_eq!(result, "improved prompt");
    }

    #[tokio::test]
    async fn rewrite_prompt_constructs_correct_request() {
        use std::sync::Mutex;

        /// Provider that captures the request for inspection.
        struct CapturingProvider {
            captured: Mutex<Option<CompletionRequest>>,
        }

        #[async_trait]
        impl Provider for CapturingProvider {
            async fn complete(
                &self,
                request: CompletionRequest,
                tx: mpsc::Sender<StreamEvent>,
            ) -> crate::provider::error::Result<()> {
                *self.captured.lock().unwrap() = Some(request);
                let _ = tx
                    .send(StreamEvent::ContentBlockDelta {
                        index: 0,
                        delta: ContentDelta::TextDelta {
                            text: "rewritten".into(),
                        },
                    })
                    .await;
                let _ = tx.send(StreamEvent::MessageStop).await;
                Ok(())
            }

            fn models(&self) -> &[Model] {
                &[]
            }

            fn name(&self) -> &str {
                "capturing"
            }
        }

        let provider = Arc::new(CapturingProvider {
            captured: Mutex::new(None),
        });
        let provider_dyn: Arc<dyn Provider> = provider.clone();

        let _ = super::rewrite_prompt(&provider_dyn, "my-model", "do the thing").await;

        let req = provider.captured.lock().unwrap().take().unwrap();
        assert_eq!(req.model, "my-model");
        assert!(req.system_prompt.is_some());
        assert!(req.system_prompt.unwrap().contains("prompt engineer"));
        assert!(req.tools.is_empty(), "rewrite call should have no tools");
        assert_eq!(req.messages.len(), 1);
        assert!(req.thinking.is_none());
    }
}
