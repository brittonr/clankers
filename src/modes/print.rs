//! One-shot print mode (-p)

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::fmt::Write as _;
use std::io::Write;
use std::sync::Arc;

use crate::agent::builder::AgentBuilder;
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
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(function_length, reason = "sequential setup/dispatch logic")
)]
pub async fn run_print_with_options(
    prompt: &str,
    provider: Arc<dyn crate::provider::Provider>,
    tools: Vec<Arc<dyn crate::tools::Tool>>,
    settings: crate::config::settings::Settings,
    model: String,
    system_prompt: String,
    opts: PrintOptions,
) -> Result<()> {
    let mut builder = AgentBuilder::new(provider, settings.clone(), model, system_prompt).with_tools(tools);
    if let Some(thinking) = opts.thinking.clone() {
        builder = builder.with_thinking(thinking);
    }
    if let Some(caps) = &settings.default_capabilities {
        let gate = std::sync::Arc::new(crate::capability_gate::UcanCapabilityGate::new(caps.clone()));
        builder = builder.with_capability_gate(gate);
    }
    let mut agent = builder.build();
    let mut rx = agent.subscribe();

    let should_show_tools = opts.show_tools;
    let should_show_stats = opts.show_stats;
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

        let mut is_in_thinking = false;

        while let Ok(event) = rx.recv().await {
            match event {
                AgentEvent::MessageUpdate {
                    delta: ContentDelta::ThinkingDelta { thinking },
                    ..
                } => {
                    if matches!(format, PrintFormat::Markdown) {
                        if !is_in_thinking {
                            writeln!(writer, "<details><summary>Thinking…</summary>\n").ok();
                            is_in_thinking = true;
                        }
                        write!(writer, "{}", thinking).ok();
                    }
                    // In plain text mode, thinking is suppressed by default
                }
                AgentEvent::MessageUpdate {
                    delta: ContentDelta::TextDelta { text },
                    ..
                } => {
                    if is_in_thinking {
                        writeln!(writer, "\n</details>\n").ok();
                        is_in_thinking = false;
                    }
                    write!(writer, "{}", text).ok();
                    writer.flush().ok();
                }
                AgentEvent::ToolCall { tool_name, .. } if should_show_tools => {
                    if is_in_thinking {
                        writeln!(writer, "\n</details>\n").ok();
                        is_in_thinking = false;
                    }
                    match format {
                        PrintFormat::Markdown => {
                            writeln!(writer, "\n**🔧 {}**", tool_name).ok();
                        }
                        PrintFormat::Text => {
                            writeln!(writer, "\n🔧 {}", tool_name).ok();
                        }
                    }
                    writer.flush().ok();
                }
                AgentEvent::ToolExecutionEnd { result, .. } if should_show_tools => {
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
                            writeln!(writer, "```\n{}\n```", text).ok();
                        }
                        PrintFormat::Text => {
                            for line in text.lines().take(20) {
                                writeln!(writer, "→ {}", line).ok();
                            }
                        }
                    }
                    writer.flush().ok();
                }
                AgentEvent::UsageUpdate {
                    turn_usage,
                    cumulative_usage,
                } if should_show_stats => {
                    let mut line = format!(
                        "[usage] turn: {}in/{}out  total: {}in/{}out",
                        turn_usage.input_tokens,
                        turn_usage.output_tokens,
                        cumulative_usage.input_tokens,
                        cumulative_usage.output_tokens,
                    );
                    if cumulative_usage.cache_read_input_tokens > 0 || cumulative_usage.cache_creation_input_tokens > 0
                    {
                        write!(
                            line,
                            "  cache: {}read/{}write",
                            cumulative_usage.cache_read_input_tokens, cumulative_usage.cache_creation_input_tokens,
                        )
                        .ok();
                    }
                    eprintln!("{}", line);
                }
                AgentEvent::AgentEnd { .. } => break,
                _ => {}
            }
        }

        // Close thinking block if still open
        if is_in_thinking {
            writeln!(writer, "\n</details>\n").ok();
        }

        writeln!(writer).ok(); // final newline
        writer.flush().ok();
    });

    agent.prompt(prompt).await?;
    print_handle.await.ok();
    Ok(())
}
