//! JSON lines event output (--mode json)

use std::io::Write;
use std::sync::Arc;

use crate::agent::Agent;
use crate::agent::events::AgentEvent;
use crate::error::Result;

/// Options for JSON output mode
#[derive(Debug, Clone, Default)]
pub struct JsonOptions {
    /// Output file (None = stdout)
    pub output_file: Option<String>,
    /// Extended thinking configuration
    pub thinking: Option<crate::provider::ThinkingConfig>,
}

/// Run JSON mode with options
pub async fn run_json_with_options(
    prompt: &str,
    provider: Arc<dyn crate::provider::Provider>,
    tools: Vec<Arc<dyn crate::tools::Tool>>,
    settings: crate::config::settings::Settings,
    model: String,
    system_prompt: String,
    opts: JsonOptions,
) -> Result<()> {
    // Snapshot model pricing before moving the provider into the agent
    let provider_models: Vec<clankers_router::Model> = provider.models().to_vec();

    let mut agent = Agent::new(provider, tools, settings.clone(), model, system_prompt);

    // Wire routing policy from settings
    if let Some(routing_config) = settings.routing.as_ref()
        && routing_config.enabled
    {
        let policy = crate::model_selection::policy::RoutingPolicy::new(routing_config.clone());
        agent = agent.with_routing_policy(policy).with_model_roles(settings.model_roles.clone());
    }

    // Wire cost tracking from settings
    if let Some(cost_config) = settings.cost_tracking.as_ref() {
        let paths = crate::config::ClankersPaths::get();
        let pricing =
            crate::model_selection::cost_tracker::pricing_from_models(&provider_models, Some(&paths.global_config_dir));
        let tracker = Arc::new(crate::model_selection::cost_tracker::CostTracker::new(pricing, cost_config.clone()));
        agent = agent.with_cost_tracker(tracker);
    }

    // Enable extended thinking if requested
    if let Some(ref thinking) = opts.thinking
        && thinking.enabled
    {
        agent.toggle_thinking(thinking.budget_tokens.unwrap_or(10_000));
    }

    let mut rx = agent.subscribe();

    let output_file = opts.output_file.clone();

    let json_handle = tokio::spawn(async move {
        let mut writer: Box<dyn Write + Send> = if let Some(ref path) = output_file {
            match std::fs::File::create(path) {
                Ok(f) => Box::new(std::io::BufWriter::new(f)),
                Err(e) => {
                    eprintln!("clankers: failed to open output file '{}': {}", path, e);
                    return;
                }
            }
        } else {
            Box::new(std::io::stdout())
        };

        while let Ok(event) = rx.recv().await {
            let json = format_event_json(&event);
            let _ = writeln!(writer, "{}", json);
            let _ = writer.flush();
            if matches!(event, AgentEvent::AgentEnd { .. }) {
                break;
            }
        }
    });

    agent.prompt(prompt).await?;
    let _ = json_handle.await;
    Ok(())
}

fn format_event_json(event: &AgentEvent) -> String {
    use serde_json::json;
    match event {
        AgentEvent::SessionStart { session_id } => {
            json!({"type": "session_start", "session_id": session_id}).to_string()
        }
        AgentEvent::AgentStart => json!({"type": "agent_start"}).to_string(),
        AgentEvent::AgentEnd { .. } => json!({"type": "agent_end"}).to_string(),
        AgentEvent::TurnStart { index } => json!({"type": "turn_start", "index": index}).to_string(),
        AgentEvent::TurnEnd { index, .. } => json!({"type": "turn_end", "index": index}).to_string(),
        AgentEvent::ContentBlockStart { index, .. } => {
            json!({"type": "content_block_start", "index": index}).to_string()
        }
        AgentEvent::ContentBlockStop { index } => json!({"type": "content_block_stop", "index": index}).to_string(),
        AgentEvent::MessageUpdate { index, delta } => {
            use crate::provider::streaming::ContentDelta;
            match delta {
                ContentDelta::TextDelta { text } => {
                    json!({"type": "text_delta", "index": index, "text": text}).to_string()
                }
                ContentDelta::ThinkingDelta { thinking } => {
                    json!({"type": "thinking_delta", "index": index, "thinking": thinking}).to_string()
                }
                ContentDelta::InputJsonDelta { partial_json } => {
                    json!({"type": "input_json_delta", "index": index, "json": partial_json}).to_string()
                }
            }
        }
        AgentEvent::ToolCall {
            tool_name,
            call_id,
            input,
        } => json!({"type": "tool_call", "tool": tool_name, "call_id": call_id, "input": input}).to_string(),
        AgentEvent::ToolExecutionStart { call_id, tool_name } => {
            json!({"type": "tool_start", "call_id": call_id, "tool": tool_name}).to_string()
        }
        AgentEvent::ToolExecutionEnd {
            call_id,
            is_error,
            result,
            ..
        } => {
            let content_text: String = result
                .content
                .iter()
                .filter_map(|c| {
                    if let crate::tools::ToolResultContent::Text { text } = c {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            json!({"type": "tool_end", "call_id": call_id, "is_error": is_error, "output": content_text}).to_string()
        }
        AgentEvent::UserInput { text, .. } => json!({"type": "user_input", "text": text}).to_string(),
        AgentEvent::UsageUpdate {
            turn_usage,
            cumulative_usage,
        } => json!({
            "type": "usage",
            "turn": {
                "input_tokens": turn_usage.input_tokens,
                "output_tokens": turn_usage.output_tokens,
                "cache_read_tokens": turn_usage.cache_read_input_tokens,
                "cache_create_tokens": turn_usage.cache_creation_input_tokens,
            },
            "cumulative": {
                "input_tokens": cumulative_usage.input_tokens,
                "output_tokens": cumulative_usage.output_tokens,
                "cache_read_tokens": cumulative_usage.cache_read_input_tokens,
                "cache_create_tokens": cumulative_usage.cache_creation_input_tokens,
            }
        })
        .to_string(),
        AgentEvent::ModelChange { from, to, reason } => {
            json!({"type": "model_change", "from": from, "to": to, "reason": reason}).to_string()
        }
        AgentEvent::SessionCompaction {
            compacted_count,
            tokens_saved,
        } => {
            json!({"type": "compaction", "compacted_count": compacted_count, "tokens_saved": tokens_saved}).to_string()
        }
        AgentEvent::BeforeAgentStart { .. } => json!({"type": "before_agent_start"}).to_string(),
        AgentEvent::UserCancel => json!({"type": "user_cancel"}).to_string(),
        AgentEvent::SessionBranch { .. } => json!({"type": "session_branch"}).to_string(),
        _ => json!({"type": "unknown"}).to_string(),
    }
}
