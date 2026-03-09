//! One-shot print mode (-p)

use std::io::Write;
use std::sync::Arc;

use crate::agent::Agent;
use crate::agent::events::AgentEvent;
use crate::error::Result;
use crate::provider::streaming::ContentDelta;

/// Options controlling headless print behaviour
#[derive(Debug, Clone, Default)]
pub struct PrintOptions {
    /// Output file (None = stdout)
    pub output_file: Option<String>,
    /// Show token usage stats at the end
    pub show_stats: bool,
    /// Show tool calls and results (verbose)
    pub show_tools: bool,
    /// Output format
    pub format: PrintFormat,
    /// Extended thinking configuration
    pub thinking: Option<crate::provider::ThinkingConfig>,
}

/// Output format for print mode
#[derive(Debug, Clone, Default)]
pub enum PrintFormat {
    /// Stream text as-is (default)
    #[default]
    Text,
    /// Wrap in Markdown (thinking blocks as `<details>`, tools as code blocks)
    Markdown,
}

/// Run print mode with full control over output behaviour
pub async fn run_print_with_options(
    prompt: &str,
    provider: Arc<dyn crate::provider::Provider>,
    tools: Vec<Arc<dyn crate::tools::Tool>>,
    settings: crate::config::settings::Settings,
    model: String,
    system_prompt: String,
    opts: PrintOptions,
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

    let show_tools = opts.show_tools;
    let show_stats = opts.show_stats;
    let format = opts.format.clone();
    let output_file = opts.output_file.clone();

    // Spawn a task to print streamed events
    let print_handle = tokio::spawn(async move {
        // Build the writer: file or stdout
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

        let mut in_thinking = false;

        while let Ok(event) = rx.recv().await {
            match event {
                AgentEvent::MessageUpdate {
                    delta: ContentDelta::ThinkingDelta { thinking },
                    ..
                } => {
                    if matches!(format, PrintFormat::Markdown) {
                        if !in_thinking {
                            let _ = writeln!(writer, "<details><summary>Thinking…</summary>\n");
                            in_thinking = true;
                        }
                        let _ = write!(writer, "{}", thinking);
                    }
                    // In plain text mode, thinking is suppressed by default
                }
                AgentEvent::MessageUpdate {
                    delta: ContentDelta::TextDelta { text },
                    ..
                } => {
                    if in_thinking {
                        let _ = writeln!(writer, "\n</details>\n");
                        in_thinking = false;
                    }
                    let _ = write!(writer, "{}", text);
                    let _ = writer.flush();
                }
                AgentEvent::ToolCall { tool_name, .. } if show_tools => {
                    if in_thinking {
                        let _ = writeln!(writer, "\n</details>\n");
                        in_thinking = false;
                    }
                    match format {
                        PrintFormat::Markdown => {
                            let _ = writeln!(writer, "\n**🔧 {}**", tool_name);
                        }
                        PrintFormat::Text => {
                            let _ = writeln!(writer, "\n🔧 {}", tool_name);
                        }
                    }
                    let _ = writer.flush();
                }
                AgentEvent::ToolExecutionEnd { result, .. } if show_tools => {
                    let text: String = result
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
                    match format {
                        PrintFormat::Markdown => {
                            let _ = writeln!(writer, "```\n{}\n```", text);
                        }
                        PrintFormat::Text => {
                            for line in text.lines().take(20) {
                                let _ = writeln!(writer, "→ {}", line);
                            }
                        }
                    }
                    let _ = writer.flush();
                }
                AgentEvent::UsageUpdate {
                    turn_usage,
                    cumulative_usage,
                } if show_stats => {
                    eprintln!(
                        "[usage] turn: {}in/{}out  total: {}in/{}out",
                        turn_usage.input_tokens,
                        turn_usage.output_tokens,
                        cumulative_usage.input_tokens,
                        cumulative_usage.output_tokens,
                    );
                }
                AgentEvent::AgentEnd { .. } => break,
                _ => {}
            }
        }

        // Close thinking block if still open
        if in_thinking {
            let _ = writeln!(writer, "\n</details>\n");
        }

        let _ = writeln!(writer); // final newline
        let _ = writer.flush();
    });

    agent.prompt(prompt).await?;
    let _ = print_handle.await;
    Ok(())
}
