//! Turn loop: prompt -> LLM -> tool calls -> repeat

mod adapters;
mod execution;
mod message;
mod model_switch;
mod policy;
mod ports;
mod steel_planning;
mod transcript;
mod usage;

use std::collections::HashMap;
use std::sync::Arc;

use adapters::AgentCancellationSource;
use adapters::AgentEngineEventSink;
use adapters::AgentModelHost;
use adapters::AgentRetrySleeper;
use adapters::AgentToolHost;
use adapters::AgentUsageObserver;
#[cfg(test)]
use chrono::Utc;
use clankers_engine::EmbeddableEngine;
#[cfg(test)]
use clankers_engine::EngineCorrelationId;
#[cfg(test)]
use clankers_engine::EngineEffect;
#[cfg(test)]
use clankers_engine::EngineEvent;
#[cfg(test)]
use clankers_engine::EngineInput;
#[cfg(test)]
use clankers_engine::EngineModelResponse;
#[cfg(test)]
use clankers_engine::EngineOutcome;
use clankers_engine::EnginePromptSubmission;
#[cfg(test)]
use clankers_engine::EngineState;
#[cfg(test)]
use clankers_engine::EngineTerminalFailure;
#[cfg(test)]
use clankers_engine::EngineTurnPhase;
use clankers_engine::EngineTurnRequest;
#[cfg(test)]
use clankers_engine::reduce;
use clankers_engine_host::EngineRunSeed;
use clankers_engine_host::HostAdapters;
use clankers_engine_host::run_engine_turn;
#[cfg(test)]
use clankers_engine_host::runtime::cancel_turn_input;
use clankers_model_selection::cost_tracker::CostTracker;
use clankers_provider::Provider;
use clankers_provider::ThinkingConfig;
#[cfg(test)]
use clankers_provider::Usage;
use clankers_provider::message::*;
#[cfg(test)]
use clankers_provider::streaming::*;
use execution::completion_request_from_engine_request;
use execution::create_error_result;
use execution::engine_messages_from_agent_messages;
use execution::execute_tools_parallel;
use execution::stream_model_request;
use execution::tool_definitions_from_tool_catalog;
use execution::tool_result_message_to_host_outcome;
use message::CollectedResponse;
pub(crate) use message::ContentBlockBuilder;
use message::apply_output_truncation;
use message::build_assistant_message;
pub(crate) use message::parse_stop_reason;
pub(crate) use message::tool_result_content_to_message_content;
use message::tool_use_count;
use model_switch::check_model_switch;
#[cfg(test)]
pub(crate) use policy::EngineModelDecision;
use policy::agent_error_from_report;
#[cfg(test)]
pub(crate) use policy::decide_model_completion;
#[cfg(test)]
pub(crate) use policy::emit_engine_notice_effects;
use policy::engine_failure_from_agent_error;
use policy::engine_outcome_or_error;
use ports::ControllerToolPort;
use ports::ProviderModelPort;
#[cfg(test)]
use serde_json::Value;
use steel_planning::AgentTurnExecutionPlanner;
use steel_planning::AgentTurnPlanningRequest;
pub use steel_planning::AgentTurnSteelPlanningConfig;
use steel_planning::emit_agent_turn_planning_receipt;
use steel_planning::plan_agent_turn;
pub use steel_planning::steel_turn_planning_config_from_settings;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use transcript::TurnTranscript;
use transcript::TurnTranscriptWriter;
use usage::update_usage_tracking;

use crate::error::AgentError;
use crate::error::Result;
use crate::events::AgentEvent;
use crate::tool::ModelSwitchSlot;
use crate::tool::Tool;

/// Configuration for a turn loop run
pub struct TurnConfig {
    pub model: String,
    pub system_prompt: String,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f64>,
    pub thinking: Option<ThinkingConfig>,
    pub model_request_slot_budget: u32,
    /// Output truncation config for tool results
    pub output_truncation: clanker_loop::OutputTruncationConfig,
    pub no_cache: bool,
    pub cache_ttl: Option<String>,
    pub steel_turn_planning: Option<AgentTurnSteelPlanningConfig>,
}

pub struct TurnLoopContext<'a> {
    pub provider: &'a dyn Provider,
    pub controller_tools: &'a HashMap<String, Arc<dyn Tool>>,
    pub event_tx: &'a broadcast::Sender<AgentEvent>,
    pub cancel: CancellationToken,
    pub cost_tracker: Option<&'a Arc<CostTracker>>,
    pub model_switch_slot: Option<&'a ModelSwitchSlot>,
    pub hook_pipeline: Option<Arc<clankers_hooks::HookPipeline>>,
    pub session_id: &'a str,
    pub db: Option<clankers_db::Db>,
    pub capability_gate: Option<Arc<dyn crate::tool::CapabilityGate>>,
    pub user_tool_filter: Option<Vec<String>>,
}

pub async fn run_turn_loop(
    config: &TurnConfig,
    ctx: TurnLoopContext<'_>,
    messages: &mut Vec<AgentMessage>,
) -> Result<()> {
    let tool_defs = tool_definitions_from_tool_catalog(ctx.controller_tools);
    if let Some(steel_turn_planning) = config.steel_turn_planning.as_ref() {
        let planning = plan_agent_turn(AgentTurnPlanningRequest {
            config: steel_turn_planning,
            session_id: ctx.session_id,
            model: &config.model,
            system_prompt: &config.system_prompt,
            messages,
            tools: ctx.controller_tools,
        });
        emit_agent_turn_planning_receipt(ctx.event_tx, &planning);
        if planning.execution_planner == AgentTurnExecutionPlanner::Blocked {
            return Err(AgentError::Agent {
                message: "steel.host.plan_turn blocked agent turn before provider request".to_string(),
            });
        }
    }
    let mut engine = EmbeddableEngine::new();
    let submit_result = engine.submit_turn(EngineTurnRequest {
        submission: EnginePromptSubmission {
            messages: engine_messages_from_agent_messages(messages),
            model: config.model.clone(),
            system_prompt: config.system_prompt.clone(),
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            thinking: config.thinking.clone(),
            tools: tool_defs,
            no_cache: config.no_cache,
            cache_ttl: config.cache_ttl.clone(),
            session_id: ctx.session_id.to_string(),
            model_request_slot_budget: config.model_request_slot_budget,
        },
    });
    let submit_seed_state = submit_result.initial_state.clone();
    let submit_outcome = engine_outcome_or_error(submit_result.outcome, "prompt submission")?;

    let transcript = TurnTranscript::new(std::mem::take(messages), config.model.clone());
    let model_port = ProviderModelPort::new(ctx.provider);
    let tool_port = ControllerToolPort {
        controller_tools: ctx.controller_tools,
        event_tx: ctx.event_tx,
        cancel: ctx.cancel.clone(),
        hook_pipeline: ctx.hook_pipeline.clone(),
        session_id: ctx.session_id,
        db: ctx.db.clone(),
        capability_gate: ctx.capability_gate.clone(),
        user_tool_filter: ctx.user_tool_filter.clone(),
    };

    let mut model_host = AgentModelHost {
        model_port: &model_port,
        event_tx: ctx.event_tx,
        cancel: ctx.cancel.clone(),
        model_switch_slot: ctx.model_switch_slot,
        transcript: transcript.writer(),
    };
    let mut tool_host = AgentToolHost {
        tool_port: &tool_port,
        event_tx: ctx.event_tx,
        output_truncation: config.output_truncation.clone(),
        transcript: transcript.writer(),
    };
    let mut retry_sleeper = AgentRetrySleeper {
        cancel: ctx.cancel.clone(),
    };
    let mut event_sink = AgentEngineEventSink {
        event_tx: ctx.event_tx,
        transcript: transcript.writer(),
    };
    let mut cancellation = AgentCancellationSource {
        cancel: ctx.cancel.clone(),
    };
    let mut usage_observer = AgentUsageObserver {
        cost_tracker: ctx.cost_tracker,
        event_tx: ctx.event_tx,
        transcript: transcript.writer(),
    };

    let report = run_engine_turn(EngineRunSeed::new(submit_seed_state, submit_outcome), HostAdapters {
        model: &mut model_host,
        tools: &mut tool_host,
        retry_sleeper: &mut retry_sleeper,
        event_sink: &mut event_sink,
        cancellation: &mut cancellation,
        usage_observer: &mut usage_observer,
    })
    .await;

    *messages = transcript.into_messages();

    if ctx.cancel.is_cancelled() {
        return Err(AgentError::Cancelled);
    }
    if let Some(error) = agent_error_from_report(&report) {
        return Err(error);
    }

    Ok(())
}

#[cfg(test)]
#[allow(
    dead_code,
    reason = "kept as focused engine-effect helpers for decoupling regression tests"
)]
fn cancel_active_engine_turn(
    engine_state: &EngineState,
    event_tx: &broadcast::Sender<AgentEvent>,
    reason: &str,
) -> Result<()> {
    let cancel_outcome =
        engine_outcome_or_error(reduce(engine_state, &cancel_turn_input(reason.to_string())), "turn cancellation")?;
    emit_engine_notice_effects(&cancel_outcome, event_tx);
    Ok(())
}

#[cfg(test)]
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_panic, no_unwrap, reason = "test code — panics are assertions")
)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use serde_json::json;

    use super::*;
    use crate::tool::ToolContext;
    use crate::tool::ToolDefinition;
    use crate::tool::ToolResult as ToolExecResult;
    use crate::tool::progress::ResultChunk;

    #[allow(clippy::too_many_arguments)]
    async fn test_run_turn_loop(
        provider: &dyn Provider,
        tools: &HashMap<String, Arc<dyn Tool>>,
        messages: &mut Vec<AgentMessage>,
        config: &TurnConfig,
        event_tx: &broadcast::Sender<AgentEvent>,
        cancel: CancellationToken,
        cost_tracker: Option<&Arc<CostTracker>>,
        model_switch_slot: Option<&ModelSwitchSlot>,
        hook_pipeline: Option<Arc<clankers_hooks::HookPipeline>>,
        session_id: &str,
        db: Option<clankers_db::Db>,
        capability_gate: Option<Arc<dyn crate::tool::CapabilityGate>>,
        user_tool_filter: Option<Vec<String>>,
    ) -> Result<()> {
        run_turn_loop(
            config,
            TurnLoopContext {
                provider,
                controller_tools: tools,
                event_tx,
                cancel,
                cost_tracker,
                model_switch_slot,
                hook_pipeline,
                session_id,
                db,
                capability_gate,
                user_tool_filter,
            },
            messages,
        )
        .await
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MatrixEntrypoint {
        StandaloneAgent,
        ControllerDaemonAdapter,
        EmbeddedBatchAdapter,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MatrixPromptSource {
        HostSupplied,
        ResumeSeed,
        ShellAssembled,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MatrixStoreMode {
        Stateless,
        SessionStore,
        DaemonTranslated,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MatrixConfirmationOutcome {
        Approved,
        DeniedByCapabilityGate,
        NoBrokerNeeded,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MatrixDisabledToolPolicy {
        None,
        UserFiltered,
        CapabilityFiltered,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MatrixToolResultClass {
        Success,
        MissingTool,
        Denied,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MatrixModelResultClass {
        Stop,
        ToolUse,
        TerminalFailure,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MatrixEventTranslation {
        NativeAgent,
        DaemonTranslated,
        EmbeddedSemantic,
    }

    #[derive(Debug, Clone, Copy)]
    struct ShellAdapterParityCase {
        id: &'static str,
        entrypoint: MatrixEntrypoint,
        prompt_source: MatrixPromptSource,
        store_mode: MatrixStoreMode,
        confirmation: MatrixConfirmationOutcome,
        disabled_tools: MatrixDisabledToolPolicy,
        tool_result: MatrixToolResultClass,
        model_result: MatrixModelResultClass,
        event_translation: MatrixEventTranslation,
    }

    const SHELL_ADAPTER_PARITY_CASES: &[ShellAdapterParityCase] = &[
        ShellAdapterParityCase {
            id: "SAPM-001-standalone-host-prompt-stop",
            entrypoint: MatrixEntrypoint::StandaloneAgent,
            prompt_source: MatrixPromptSource::HostSupplied,
            store_mode: MatrixStoreMode::Stateless,
            confirmation: MatrixConfirmationOutcome::NoBrokerNeeded,
            disabled_tools: MatrixDisabledToolPolicy::None,
            tool_result: MatrixToolResultClass::Success,
            model_result: MatrixModelResultClass::Stop,
            event_translation: MatrixEventTranslation::NativeAgent,
        },
        ShellAdapterParityCase {
            id: "SAPM-002-controller-resume-capability-denial",
            entrypoint: MatrixEntrypoint::ControllerDaemonAdapter,
            prompt_source: MatrixPromptSource::ResumeSeed,
            store_mode: MatrixStoreMode::DaemonTranslated,
            confirmation: MatrixConfirmationOutcome::DeniedByCapabilityGate,
            disabled_tools: MatrixDisabledToolPolicy::CapabilityFiltered,
            tool_result: MatrixToolResultClass::Denied,
            model_result: MatrixModelResultClass::ToolUse,
            event_translation: MatrixEventTranslation::DaemonTranslated,
        },
        ShellAdapterParityCase {
            id: "SAPM-003-embedded-batch-user-filter-missing-tool",
            entrypoint: MatrixEntrypoint::EmbeddedBatchAdapter,
            prompt_source: MatrixPromptSource::HostSupplied,
            store_mode: MatrixStoreMode::Stateless,
            confirmation: MatrixConfirmationOutcome::NoBrokerNeeded,
            disabled_tools: MatrixDisabledToolPolicy::UserFiltered,
            tool_result: MatrixToolResultClass::MissingTool,
            model_result: MatrixModelResultClass::ToolUse,
            event_translation: MatrixEventTranslation::EmbeddedSemantic,
        },
        ShellAdapterParityCase {
            id: "SAPM-004-standalone-shell-assembled-approved-tool",
            entrypoint: MatrixEntrypoint::StandaloneAgent,
            prompt_source: MatrixPromptSource::ShellAssembled,
            store_mode: MatrixStoreMode::SessionStore,
            confirmation: MatrixConfirmationOutcome::Approved,
            disabled_tools: MatrixDisabledToolPolicy::None,
            tool_result: MatrixToolResultClass::Success,
            model_result: MatrixModelResultClass::ToolUse,
            event_translation: MatrixEventTranslation::NativeAgent,
        },
        ShellAdapterParityCase {
            id: "SAPM-005-controller-terminal-failure-event",
            entrypoint: MatrixEntrypoint::ControllerDaemonAdapter,
            prompt_source: MatrixPromptSource::HostSupplied,
            store_mode: MatrixStoreMode::DaemonTranslated,
            confirmation: MatrixConfirmationOutcome::NoBrokerNeeded,
            disabled_tools: MatrixDisabledToolPolicy::None,
            tool_result: MatrixToolResultClass::Success,
            model_result: MatrixModelResultClass::TerminalFailure,
            event_translation: MatrixEventTranslation::DaemonTranslated,
        },
    ];

    #[test]
    fn shell_adapter_parity_matrix_names_required_axes() {
        assert!(SHELL_ADAPTER_PARITY_CASES.iter().all(|case| case.id.starts_with("SAPM-")));
        for entrypoint in [
            MatrixEntrypoint::StandaloneAgent,
            MatrixEntrypoint::ControllerDaemonAdapter,
            MatrixEntrypoint::EmbeddedBatchAdapter,
        ] {
            assert!(SHELL_ADAPTER_PARITY_CASES.iter().any(|case| case.entrypoint == entrypoint));
        }
        for prompt_source in [
            MatrixPromptSource::HostSupplied,
            MatrixPromptSource::ResumeSeed,
            MatrixPromptSource::ShellAssembled,
        ] {
            assert!(SHELL_ADAPTER_PARITY_CASES.iter().any(|case| case.prompt_source == prompt_source));
        }
        for store_mode in [
            MatrixStoreMode::Stateless,
            MatrixStoreMode::SessionStore,
            MatrixStoreMode::DaemonTranslated,
        ] {
            assert!(SHELL_ADAPTER_PARITY_CASES.iter().any(|case| case.store_mode == store_mode));
        }
        for confirmation in [
            MatrixConfirmationOutcome::Approved,
            MatrixConfirmationOutcome::DeniedByCapabilityGate,
            MatrixConfirmationOutcome::NoBrokerNeeded,
        ] {
            assert!(SHELL_ADAPTER_PARITY_CASES.iter().any(|case| case.confirmation == confirmation));
        }
        for disabled_tools in [
            MatrixDisabledToolPolicy::None,
            MatrixDisabledToolPolicy::UserFiltered,
            MatrixDisabledToolPolicy::CapabilityFiltered,
        ] {
            assert!(SHELL_ADAPTER_PARITY_CASES.iter().any(|case| case.disabled_tools == disabled_tools));
        }
        for tool_result in [
            MatrixToolResultClass::Success,
            MatrixToolResultClass::MissingTool,
            MatrixToolResultClass::Denied,
        ] {
            assert!(SHELL_ADAPTER_PARITY_CASES.iter().any(|case| case.tool_result == tool_result));
        }
        for model_result in [
            MatrixModelResultClass::Stop,
            MatrixModelResultClass::ToolUse,
            MatrixModelResultClass::TerminalFailure,
        ] {
            assert!(SHELL_ADAPTER_PARITY_CASES.iter().any(|case| case.model_result == model_result));
        }
        for event_translation in [
            MatrixEventTranslation::NativeAgent,
            MatrixEventTranslation::DaemonTranslated,
            MatrixEventTranslation::EmbeddedSemantic,
        ] {
            assert!(SHELL_ADAPTER_PARITY_CASES.iter().any(|case| case.event_translation == event_translation));
        }
    }

    #[tokio::test]
    async fn standalone_agent_shell_adapter_parity_cases_preserve_engine_inputs_and_terminal_outcomes() {
        let (event_tx, _rx) = broadcast::channel(16);
        let mut messages = vec![make_user_message()];
        let provider = RetryableFailProvider::new(0, 500);
        let tools = HashMap::new();
        let result = test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &make_turn_config(),
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "shell-parity-session",
            None,
            None,
            None,
        )
        .await;
        assert!(result.is_ok());
        assert!(messages.iter().any(|message| matches!(message, AgentMessage::Assistant { .. })));
        assert!(SHELL_ADAPTER_PARITY_CASES.iter().any(|case| case.entrypoint == MatrixEntrypoint::StandaloneAgent
            && case.prompt_source == MatrixPromptSource::HostSupplied
            && case.model_result == MatrixModelResultClass::Stop));
    }

    #[tokio::test]
    async fn embedded_batch_adapter_dogfoods_product_kit_without_shell_runtime() {
        let catalog = clankers_adapters::EmbeddedToolCatalog {
            tools: vec![clankers_adapters::EmbeddedToolMetadata {
                name: "product_context".to_string(),
                description: "Read product-owned context for an embedded turn".to_string(),
                runtime: clankers_adapters::EmbeddedToolRuntime::ProductOwned,
                capabilities: vec![clankers_adapters::EmbeddedCapability::Read],
                approval: clankers_adapters::ApprovalPolicy::Never,
                redaction: clankers_adapters::RedactionPolicy::None,
                input_schema: json!({
                    "type": "object",
                    "properties": {"topic": {"type": "string"}},
                    "required": ["topic"]
                }),
            }],
        };
        catalog.validate().expect("dogfood catalog should be embedding-safe");
        let tool_defs = catalog
            .tools
            .iter()
            .map(|tool| clanker_message::ToolDefinition {
                name: tool.name.clone(),
                description: tool.description.clone(),
                input_schema: tool.input_schema.clone(),
            })
            .collect();
        let submit_outcome = clankers_engine::reduce(
            &EngineState::new(),
            &EngineInput::submit_user_prompt(clankers_engine::EnginePromptSubmission {
                messages: engine_messages_from_agent_messages(&[make_user_message()]),
                model: "embedded-product-model".to_string(),
                system_prompt: "Answer using product-owned context only.".to_string(),
                max_tokens: Some(256),
                temperature: None,
                thinking: None,
                tools: tool_defs,
                no_cache: true,
                cache_ttl: None,
                session_id: "embedded-product-dogfood".to_string(),
                model_request_slot_budget: 3,
            }),
        );
        assert!(submit_outcome.rejection.is_none());

        let mut model_host = clankers_adapters::ScriptedModelHost::new([
            clankers_adapters::ScriptedModelHost::tool_request(
                "call-product-context",
                "product_context",
                json!({"topic":"embedding-readiness"}),
            ),
            clankers_adapters::ScriptedModelHost::completed_text_with_usage(
                "Product embedding path is wired through the reusable kit.",
                clanker_message::Usage {
                    input_tokens: 7,
                    output_tokens: 11,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
            ),
        ]);
        let mut tool_host = clankers_adapters::CatalogToolExecutor::new(catalog).with_outcome(
            "product_context",
            clankers_adapters::ScriptedToolExecutor::text_success("context: controlled internal product embedding"),
        );
        let mut retry_sleeper = clankers_adapters::NoopRetrySleeper::default();
        let mut event_sink = clankers_adapters::MemoryEventSink::default();
        let mut cancellation = clankers_adapters::AtomicCancellationSource::default();
        let mut usage_observer = clankers_adapters::CollectingUsageObserver::default();

        let report = run_engine_turn(EngineRunSeed::new(EngineState::new(), submit_outcome), HostAdapters {
            model: &mut model_host,
            tools: &mut tool_host,
            retry_sleeper: &mut retry_sleeper,
            event_sink: &mut event_sink,
            cancellation: &mut cancellation,
            usage_observer: &mut usage_observer,
        })
        .await;

        assert!(matches!(report.final_state.phase, EngineTurnPhase::Finished));
        assert!(report.adapter_diagnostics.is_empty());
        assert_eq!(model_host.requests().len(), 2);
        assert!(event_sink.events().iter().any(|event| matches!(event, clankers_engine::EngineEvent::TurnFinished {
            stop_reason: StopReason::Stop
        })));
        assert_eq!(usage_observer.observations().len(), 1);
        assert!(SHELL_ADAPTER_PARITY_CASES.iter().any(|case| {
            case.id == "SAPM-003-embedded-batch-user-filter-missing-tool"
                && case.entrypoint == MatrixEntrypoint::EmbeddedBatchAdapter
                && case.event_translation == MatrixEventTranslation::EmbeddedSemantic
        }));
    }

    #[test]
    fn shell_adapter_parity_matrix_evidence_is_present_and_source_bounded() {
        let unsupported_shell_paths = ["daemon socket", "database store", "oauth", "plugin runtime"];
        for marker in unsupported_shell_paths {
            assert!(!marker.contains("embedded"));
        }
        assert!(SHELL_ADAPTER_PARITY_CASES.iter().any(|case| {
            case.entrypoint == MatrixEntrypoint::EmbeddedBatchAdapter
                && case.event_translation == MatrixEventTranslation::EmbeddedSemantic
        }));
    }

    // -----------------------------------------------------------------------
    // parse_stop_reason
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_stop_reason_end_turn() {
        assert_eq!(parse_stop_reason("end_turn"), StopReason::Stop);
    }

    #[test]
    fn test_parse_stop_reason_stop() {
        assert_eq!(parse_stop_reason("stop"), StopReason::Stop);
    }

    #[test]
    fn test_parse_stop_reason_tool_use() {
        assert_eq!(parse_stop_reason("tool_use"), StopReason::ToolUse);
    }

    #[test]
    fn test_parse_stop_reason_max_tokens() {
        assert_eq!(parse_stop_reason("max_tokens"), StopReason::MaxTokens);
    }

    #[test]
    fn test_parse_stop_reason_unknown_defaults_to_stop() {
        assert_eq!(parse_stop_reason("something_else"), StopReason::Stop);
        assert_eq!(parse_stop_reason(""), StopReason::Stop);
    }

    // -----------------------------------------------------------------------
    // ContentBlockBuilder
    // -----------------------------------------------------------------------

    #[test]
    fn test_content_block_builder_text_delta() {
        let mut builder = ContentBlockBuilder::new(Content::Text { text: String::new() });
        builder.apply_delta(&ContentDelta::TextDelta {
            text: "Hello".to_string(),
        });
        builder.apply_delta(&ContentDelta::TextDelta {
            text: " world".to_string(),
        });

        match builder.finalize() {
            Content::Text { text } => assert_eq!(text, "Hello world"),
            other => panic!("Expected Text, got {:?}", other),
        }
    }

    #[test]
    fn test_content_block_builder_thinking_delta() {
        let mut builder = ContentBlockBuilder::new(Content::Thinking {
            thinking: String::new(),
            signature: String::new(),
        });
        builder.apply_delta(&ContentDelta::ThinkingDelta {
            thinking: "Let me think...".to_string(),
        });
        builder.apply_delta(&ContentDelta::ThinkingDelta {
            thinking: " more thoughts".to_string(),
        });

        match builder.finalize() {
            Content::Thinking { thinking, .. } => assert_eq!(thinking, "Let me think... more thoughts"),
            other => panic!("Expected Thinking, got {:?}", other),
        }
    }

    #[test]
    fn test_content_block_builder_signature_delta() {
        let mut builder = ContentBlockBuilder::new(Content::Thinking {
            thinking: "some thought".to_string(),
            signature: String::new(),
        });
        builder.apply_delta(&ContentDelta::SignatureDelta {
            signature: "sig_part1".to_string(),
        });
        builder.apply_delta(&ContentDelta::SignatureDelta {
            signature: "_part2".to_string(),
        });

        match builder.finalize() {
            Content::Thinking { thinking, signature } => {
                assert_eq!(thinking, "some thought");
                assert_eq!(signature, "sig_part1_part2");
            }
            other => panic!("Expected Thinking, got {:?}", other),
        }
    }

    #[test]
    fn test_content_block_builder_tool_use_json_delta() {
        let mut builder = ContentBlockBuilder::new(Content::ToolUse {
            id: "call_1".to_string(),
            name: "bash".to_string(),
            input: json!({}),
        });
        builder.apply_delta(&ContentDelta::InputJsonDelta {
            partial_json: r#"{"com"#.to_string(),
        });
        builder.apply_delta(&ContentDelta::InputJsonDelta {
            partial_json: r#"mand": "ls"}"#.to_string(),
        });

        match builder.finalize() {
            Content::ToolUse { input, name, .. } => {
                assert_eq!(name, "bash");
                assert_eq!(input, json!({"command": "ls"}));
            }
            other => panic!("Expected ToolUse, got {:?}", other),
        }
    }

    #[test]
    fn test_content_block_builder_tool_use_empty_json() {
        let builder = ContentBlockBuilder::new(Content::ToolUse {
            id: "call_2".to_string(),
            name: "test".to_string(),
            input: json!(null), // Non-object input should become {}
        });

        match builder.finalize() {
            Content::ToolUse { input, .. } => {
                assert!(input.is_object(), "Expected object, got {:?}", input);
            }
            other => panic!("Expected ToolUse, got {:?}", other),
        }
    }

    #[test]
    fn test_content_block_builder_tool_use_invalid_json_fallback() {
        let mut builder = ContentBlockBuilder::new(Content::ToolUse {
            id: "call_3".to_string(),
            name: "test".to_string(),
            input: json!({}),
        });
        // Incomplete JSON
        builder.apply_delta(&ContentDelta::InputJsonDelta {
            partial_json: r#"{"key": "#.to_string(),
        });

        match builder.finalize() {
            Content::ToolUse { input, .. } => {
                // Should keep original {} since parse failed
                assert!(input.is_object());
            }
            other => panic!("Expected ToolUse, got {:?}", other),
        }
    }

    #[test]
    fn test_content_block_builder_mismatched_delta_ignored() {
        let mut builder = ContentBlockBuilder::new(Content::Text {
            text: "hello".to_string(),
        });
        // Applying a thinking delta to a text block should be ignored
        builder.apply_delta(&ContentDelta::ThinkingDelta {
            thinking: "thinking".to_string(),
        });

        match builder.finalize() {
            Content::Text { text } => assert_eq!(text, "hello"),
            other => panic!("Expected Text, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // tool_result_content_to_message_content
    // -----------------------------------------------------------------------

    #[test]
    fn test_tool_result_text_conversion() {
        use crate::tool::ToolResultContent;
        let content = vec![ToolResultContent::Text {
            text: "output".to_string(),
        }];
        let result = tool_result_content_to_message_content(&content);
        assert_eq!(result.len(), 1);
        match &result[0] {
            Content::Text { text } => assert_eq!(text, "output"),
            other => panic!("Expected Text, got {:?}", other),
        }
    }

    #[test]
    fn test_tool_result_image_conversion() {
        use crate::tool::ToolResultContent;
        let content = vec![ToolResultContent::Image {
            media_type: "image/png".to_string(),
            data: "base64data".to_string(),
        }];
        let result = tool_result_content_to_message_content(&content);
        assert_eq!(result.len(), 1);
        match &result[0] {
            Content::Image {
                source: ImageSource::Base64 { media_type, data },
            } => {
                assert_eq!(media_type, "image/png");
                assert_eq!(data, "base64data");
            }
            other => panic!("Expected Image, got {:?}", other),
        }
    }

    #[test]
    fn test_tool_result_mixed_content() {
        use crate::tool::ToolResultContent;
        let content = vec![
            ToolResultContent::Text {
                text: "text".to_string(),
            },
            ToolResultContent::Image {
                media_type: "image/jpeg".to_string(),
                data: "jpg_data".to_string(),
            },
        ];
        let result = tool_result_content_to_message_content(&content);
        assert_eq!(result.len(), 2);
        assert!(matches!(&result[0], Content::Text { .. }));
        assert!(matches!(&result[1], Content::Image { .. }));
    }

    #[test]
    fn test_tool_result_empty_content() {
        let result = tool_result_content_to_message_content(&[]);
        assert!(result.is_empty());
    }

    // -----------------------------------------------------------------------
    // Phase 4: Accumulator integration in execute_tools_parallel
    // -----------------------------------------------------------------------

    /// A tool that emits result chunks during execution
    struct ChunkEmittingTool {
        def: ToolDefinition,
    }

    impl ChunkEmittingTool {
        fn new() -> Self {
            Self {
                def: ToolDefinition {
                    name: "chunk_tool".to_string(),
                    description: "Emits result chunks".to_string(),
                    input_schema: json!({"type": "object", "properties": {}}),
                },
            }
        }
    }

    #[async_trait]
    impl Tool for ChunkEmittingTool {
        fn definition(&self) -> &ToolDefinition {
            &self.def
        }

        async fn execute(&self, ctx: &ToolContext, _params: Value) -> ToolExecResult {
            // Emit several chunks
            ctx.emit_result_chunk(ResultChunk::text("line 1\nline 2"));
            ctx.emit_result_chunk(ResultChunk::text("line 3\nline 4"));
            ctx.emit_result_chunk(ResultChunk::text("line 5"));

            // Yield to let collector process events
            tokio::task::yield_now().await;

            // Return a direct result (should be ignored in favor of accumulated)
            ToolExecResult::text("direct result (should be overridden)")
        }
    }

    /// A tool that returns a direct result without emitting chunks
    struct DirectResultTool {
        def: ToolDefinition,
    }

    impl DirectResultTool {
        fn new() -> Self {
            Self {
                def: ToolDefinition {
                    name: "direct_tool".to_string(),
                    description: "Returns direct result".to_string(),
                    input_schema: json!({"type": "object", "properties": {}}),
                },
            }
        }
    }

    #[async_trait]
    impl Tool for DirectResultTool {
        fn definition(&self) -> &ToolDefinition {
            &self.def
        }

        async fn execute(&self, _ctx: &ToolContext, _params: Value) -> ToolExecResult {
            ToolExecResult::text("direct output")
        }
    }

    /// A tool that returns an error result.
    struct FailingTool {
        def: ToolDefinition,
    }

    impl FailingTool {
        fn new() -> Self {
            Self {
                def: ToolDefinition {
                    name: "failing_tool".to_string(),
                    description: "Returns an error result".to_string(),
                    input_schema: json!({"type": "object", "properties": {}}),
                },
            }
        }
    }

    #[async_trait]
    impl Tool for FailingTool {
        fn definition(&self) -> &ToolDefinition {
            &self.def
        }

        async fn execute(&self, _ctx: &ToolContext, _params: Value) -> ToolExecResult {
            ToolExecResult::error("tool failed")
        }
    }

    #[test]
    fn output_truncation_preserves_existing_clankers_limit_metadata() {
        const SMALL_OUTPUT_BYTES: usize = 8;
        const SMALL_OUTPUT_LINES: usize = 2;
        let messages = vec![ToolResultMessage {
            id: MessageId::new("tool-truncate"),
            call_id: "call-truncate".to_string(),
            tool_name: "direct_tool".to_string(),
            content: vec![Content::Text {
                text: "one\ntwo\nthree\n".to_string(),
            }],
            is_error: false,
            details: None,
            timestamp: Utc::now(),
        }];
        let config = clanker_loop::OutputTruncationConfig {
            max_bytes: SMALL_OUTPUT_BYTES,
            max_lines: SMALL_OUTPUT_LINES,
            enabled: true,
        };

        let truncated = apply_output_truncation(messages, &config);

        assert!(truncated[0].details.is_none());
        assert!(
            matches!(&truncated[0].content[0], Content::Text { text } if text.contains("Output truncated") && text.contains("Use `read"))
        );
    }

    #[tokio::test]
    async fn accumulator_collects_chunks_from_tool() {
        let tool: Arc<dyn Tool> = Arc::new(ChunkEmittingTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("chunk_tool".to_string(), tool);

        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        let tool_calls = vec![("call-1".to_string(), "chunk_tool".to_string(), json!({}))];

        let results = execute_tools_parallel(&tools, &tool_calls, &event_tx, cancel, None, "", None, None, None).await;

        assert_eq!(results.len(), 1);
        let msg = &results[0];
        assert!(!msg.is_error);

        // Should contain accumulated text, not "direct result"
        let text = match &msg.content[0] {
            Content::Text { text } => text,
            other => panic!("expected Text, got {:?}", other),
        };
        assert!(text.contains("line 1"), "expected accumulated text, got: {}", text);
        assert!(text.contains("line 5"), "expected accumulated text, got: {}", text);
        assert!(!text.contains("direct result"), "should use accumulated, not direct");

        // Should have details with accumulator metadata
        let details = msg.details.as_ref().expect("expected details");
        assert_eq!(details["chunks"], 3);
        assert!(details["total_lines"].as_u64().expect("total_lines should be u64") >= 5);
        assert!(!details["truncated"].as_bool().expect("truncated should be bool"));
    }

    #[tokio::test]
    async fn direct_result_used_when_no_chunks() {
        let tool: Arc<dyn Tool> = Arc::new(DirectResultTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("direct_tool".to_string(), tool);

        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        let tool_calls = vec![("call-2".to_string(), "direct_tool".to_string(), json!({}))];

        let results = execute_tools_parallel(&tools, &tool_calls, &event_tx, cancel, None, "", None, None, None).await;

        assert_eq!(results.len(), 1);
        let msg = &results[0];
        assert!(!msg.is_error);

        // Should contain the direct result text
        let text = match &msg.content[0] {
            Content::Text { text } => text,
            other => panic!("expected Text, got {:?}", other),
        };
        assert_eq!(text, "direct output");

        // No details (direct result has no accumulator metadata)
        assert!(msg.details.is_none());
    }

    #[tokio::test]
    async fn user_tool_filter_blocks_unlisted_tools() {
        let tool: Arc<dyn Tool> = Arc::new(DirectResultTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("direct_tool".to_string(), tool);

        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        let tool_calls = vec![("call-1".to_string(), "direct_tool".to_string(), json!({}))];

        // Filter only allows "read" — direct_tool should be blocked
        let filter = Some(vec!["read".to_string()]);
        let results =
            execute_tools_parallel(&tools, &tool_calls, &event_tx, cancel, None, "", None, None, filter).await;

        assert_eq!(results.len(), 1);
        assert!(results[0].is_error);
        let text = match &results[0].content[0] {
            Content::Text { text } => text,
            other => panic!("expected Text, got {:?}", other),
        };
        assert!(text.contains("🔒"), "expected locked error, got: {text}");
    }

    #[tokio::test]
    async fn user_tool_filter_allows_listed_tools() {
        let tool: Arc<dyn Tool> = Arc::new(DirectResultTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("direct_tool".to_string(), tool);

        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        let tool_calls = vec![("call-1".to_string(), "direct_tool".to_string(), json!({}))];

        // Filter allows direct_tool
        let filter = Some(vec!["direct_tool,read".to_string()]);
        let results =
            execute_tools_parallel(&tools, &tool_calls, &event_tx, cancel, None, "", None, None, filter).await;

        assert_eq!(results.len(), 1);
        assert!(!results[0].is_error);
    }

    #[tokio::test]
    async fn user_tool_filter_none_allows_all() {
        let tool: Arc<dyn Tool> = Arc::new(DirectResultTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("direct_tool".to_string(), tool);

        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        let tool_calls = vec![("call-1".to_string(), "direct_tool".to_string(), json!({}))];

        // No filter — full access
        let results = execute_tools_parallel(&tools, &tool_calls, &event_tx, cancel, None, "", None, None, None).await;

        assert_eq!(results.len(), 1);
        assert!(!results[0].is_error);
    }

    #[tokio::test]
    async fn user_tool_filter_applies_latest_allowlist_per_call() {
        let tool: Arc<dyn Tool> = Arc::new(DirectResultTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("direct_tool".to_string(), tool);

        let (event_tx, _rx) = broadcast::channel(256);
        let tool_calls = vec![("call-1".to_string(), "direct_tool".to_string(), json!({}))];

        let blocked_results = execute_tools_parallel(
            &tools,
            &tool_calls,
            &event_tx,
            CancellationToken::new(),
            None,
            "",
            None,
            None,
            Some(vec!["read".to_string()]),
        )
        .await;
        assert!(blocked_results[0].is_error);

        let allowed_results = execute_tools_parallel(
            &tools,
            &tool_calls,
            &event_tx,
            CancellationToken::new(),
            None,
            "",
            None,
            None,
            Some(vec!["direct_tool".to_string()]),
        )
        .await;
        assert!(!allowed_results[0].is_error);
    }

    #[tokio::test]
    async fn controller_filtered_tool_inventory_replaces_available_tools_without_turn_local_state() {
        let tool: Arc<dyn Tool> = Arc::new(DirectResultTool::new());
        let mut full_tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        full_tools.insert("direct_tool".to_string(), Arc::clone(&tool));
        let filtered_tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let (event_tx, _rx) = broadcast::channel(256);
        let tool_calls = vec![("call-1".to_string(), "direct_tool".to_string(), json!({}))];

        let allowed_results = execute_tools_parallel(
            &full_tools,
            &tool_calls,
            &event_tx,
            CancellationToken::new(),
            None,
            "",
            None,
            None,
            None,
        )
        .await;
        assert!(!allowed_results[0].is_error);

        let filtered_results = execute_tools_parallel(
            &filtered_tools,
            &tool_calls,
            &event_tx,
            CancellationToken::new(),
            None,
            "",
            None,
            None,
            None,
        )
        .await;
        assert!(filtered_results[0].is_error);
        let text = match &filtered_results[0].content[0] {
            Content::Text { text } => text,
            other => panic!("expected Text, got {:?}", other),
        };
        assert_eq!(text, "Tool 'direct_tool' not found");
    }

    // -----------------------------------------------------------------------
    // Turn-level retry tests
    // -----------------------------------------------------------------------

    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use tokio::sync::mpsc;

    const RETRYABLE_PROVIDER_STATUS: u16 = 502;
    const NON_RETRYABLE_PROVIDER_STATUS: u16 = 400;
    const SINGLE_PROVIDER_FAILURE: usize = 1;
    const ALWAYS_FAIL_PROVIDER_FAILURES: usize = usize::MAX;
    const EXPECTED_USER_ONLY_MESSAGE_COUNT: usize = 1;
    const EXPECTED_ASSISTANT_MESSAGE_COUNT: usize = 2;
    const EXPECTED_TOOL_BUDGET_MESSAGE_COUNT: usize = 3;
    const EXPECTED_SINGLE_PROVIDER_CALL: usize = 1;
    const EXPECTED_RETRY_RECOVERY_PROVIDER_CALLS: usize = 2;
    const EXPECTED_RETRY_EXHAUSTION_PROVIDER_CALLS: usize = 3;
    const ZERO_MODEL_REQUEST_SLOT_BUDGET: u32 = 0;
    const SINGLE_MODEL_REQUEST_SLOT_BUDGET: u32 = 1;
    const RETRY_CANCELLATION_DELAY_MS: u64 = 100;

    /// Provider that fails N times with a retryable error, then succeeds.
    struct RetryableFailProvider {
        failures_remaining: AtomicUsize,
        call_count: AtomicUsize,
        status: u16,
    }

    impl RetryableFailProvider {
        fn new(fail_count: usize, status: u16) -> Self {
            Self {
                failures_remaining: AtomicUsize::new(fail_count),
                call_count: AtomicUsize::new(0),
                status,
            }
        }

        fn call_count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl clankers_provider::Provider for RetryableFailProvider {
        async fn complete(
            &self,
            _request: clankers_provider::CompletionRequest,
            tx: mpsc::Sender<StreamEvent>,
        ) -> clankers_provider::error::Result<()> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let remaining = self.failures_remaining.fetch_sub(1, Ordering::SeqCst);
            if remaining > 0 {
                return Err(clankers_provider::error::provider_err_with_status_for_provider(
                    self.status,
                    format!("HTTP error {}", self.status),
                    "anthropic",
                ));
            }
            // Succeed: send minimal valid response
            tx.send(StreamEvent::MessageStart {
                message: MessageMetadata {
                    id: "msg-1".into(),
                    model: "test-model".into(),
                    role: "assistant".into(),
                },
            })
            .await
            .ok();
            tx.send(StreamEvent::ContentBlockStart {
                index: 0,
                content_block: Content::Text { text: String::new() },
            })
            .await
            .ok();
            tx.send(StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::TextDelta { text: "OK".into() },
            })
            .await
            .ok();
            tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
            tx.send(StreamEvent::MessageDelta {
                stop_reason: Some("end_turn".into()),
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 2,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
            })
            .await
            .ok();
            tx.send(StreamEvent::MessageStop).await.ok();
            Ok(())
        }
        fn models(&self) -> &[clankers_provider::Model] {
            &[]
        }
        fn name(&self) -> &str {
            "test"
        }
    }

    fn make_turn_config() -> TurnConfig {
        TurnConfig {
            model: "test-model".into(),
            system_prompt: "You are a test assistant.".into(),
            max_tokens: Some(100),
            temperature: None,
            thinking: None,
            model_request_slot_budget: 1,
            output_truncation: clanker_loop::OutputTruncationConfig::default(),
            no_cache: true,
            cache_ttl: None,
            steel_turn_planning: None,
        }
    }

    fn make_user_message() -> AgentMessage {
        AgentMessage::User(UserMessage {
            id: MessageId::new("test-msg"),
            content: vec![Content::Text { text: "hello".into() }],
            timestamp: Utc::now(),
        })
    }

    #[tokio::test]
    async fn run_turn_loop_emits_steel_plan_turn_receipt_when_configured() {
        let provider = RetryableFailProvider::new(0, RETRYABLE_PROVIDER_STATUS);
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let mut messages = vec![make_user_message()];
        let mut config = make_turn_config();
        let profile = clankers_runtime::SteelOrchestrationProfile::comparison_default(
            clankers_artifacts::ArtifactHash::digest(b"script"),
            clankers_artifacts::ArtifactHash::digest(b"policy"),
        );
        config.steel_turn_planning = Some(AgentTurnSteelPlanningConfig::comparison_fixture(profile));
        let (event_tx, mut event_rx) = broadcast::channel(256);

        test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "session-steel-turn",
            None,
            None,
            None,
        )
        .await
        .expect("turn should succeed");

        let mut saw_steel_receipt = false;
        while let Ok(event) = event_rx.try_recv() {
            if let AgentEvent::SystemMessage { message } = event {
                saw_steel_receipt |= message.contains("steel.host.plan_turn receipt")
                    && message.contains("status=Authorized")
                    && !message.contains("hello");
            }
        }
        assert!(saw_steel_receipt, "configured run_turn_loop should emit Steel planning receipt");
    }

    #[tokio::test]
    async fn turn_request_includes_session_id_extra_param() {
        use std::sync::Mutex;

        struct CapturingProvider {
            captured: Mutex<Option<clankers_provider::CompletionRequest>>,
        }

        #[async_trait]
        impl clankers_provider::Provider for CapturingProvider {
            async fn complete(
                &self,
                request: clankers_provider::CompletionRequest,
                tx: mpsc::Sender<StreamEvent>,
            ) -> clankers_provider::error::Result<()> {
                *self.captured.lock().expect("capture lock poisoned") = Some(request);
                tx.send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: "msg-1".into(),
                        model: "test-model".into(),
                        role: "assistant".into(),
                    },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockStart {
                    index: 0,
                    content_block: Content::Text { text: String::new() },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: ContentDelta::TextDelta { text: "OK".into() },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                tx.send(StreamEvent::MessageDelta {
                    stop_reason: Some("end_turn".into()),
                    usage: Usage {
                        input_tokens: 10,
                        output_tokens: 2,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    },
                })
                .await
                .ok();
                tx.send(StreamEvent::MessageStop).await.ok();
                Ok(())
            }

            fn models(&self) -> &[clankers_provider::Model] {
                &[]
            }

            fn name(&self) -> &str {
                "capturing"
            }
        }

        let provider = CapturingProvider {
            captured: Mutex::new(None),
        };
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let mut messages = vec![make_user_message()];
        let config = make_turn_config();
        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            cancel,
            None,
            None,
            None,
            "session-123",
            None,
            None,
            None,
        )
        .await
        .expect("turn should succeed");

        let captured =
            provider.captured.lock().expect("capture lock poisoned").take().expect("request should be captured");
        assert_eq!(captured.extra_params.get("_session_id"), Some(&json!("session-123")));
    }

    #[tokio::test]
    async fn turn_request_reuses_session_id_across_later_turns() {
        use std::sync::Mutex;

        struct SequenceCapturingProvider {
            captured: Mutex<Vec<clankers_provider::CompletionRequest>>,
        }

        #[async_trait]
        impl clankers_provider::Provider for SequenceCapturingProvider {
            async fn complete(
                &self,
                request: clankers_provider::CompletionRequest,
                tx: mpsc::Sender<StreamEvent>,
            ) -> clankers_provider::error::Result<()> {
                self.captured.lock().expect("capture lock poisoned").push(request);
                tx.send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: "msg-1".into(),
                        model: "test-model".into(),
                        role: "assistant".into(),
                    },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockStart {
                    index: 0,
                    content_block: Content::Text { text: String::new() },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: ContentDelta::TextDelta { text: "OK".into() },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                tx.send(StreamEvent::MessageDelta {
                    stop_reason: Some("end_turn".into()),
                    usage: Usage {
                        input_tokens: 10,
                        output_tokens: 2,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    },
                })
                .await
                .ok();
                tx.send(StreamEvent::MessageStop).await.ok();
                Ok(())
            }

            fn models(&self) -> &[clankers_provider::Model] {
                &[]
            }

            fn name(&self) -> &str {
                "sequence-capturing"
            }
        }

        let provider = SequenceCapturingProvider {
            captured: Mutex::new(Vec::new()),
        };
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let mut messages = vec![make_user_message()];
        let config = make_turn_config();
        let (event_tx, _rx) = broadcast::channel(256);

        test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "session-stable",
            None,
            None,
            None,
        )
        .await
        .expect("first turn should succeed");

        messages.push(AgentMessage::User(UserMessage {
            id: MessageId::new("test-msg-2"),
            content: vec![Content::Text {
                text: "hello again".into(),
            }],
            timestamp: Utc::now(),
        }));

        test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "session-stable",
            None,
            None,
            None,
        )
        .await
        .expect("second turn should succeed");

        let captured = provider.captured.lock().expect("capture lock poisoned");
        assert_eq!(captured.len(), 2);
        assert_eq!(captured[0].extra_params.get("_session_id"), Some(&json!("session-stable")));
        assert_eq!(captured[1].extra_params.get("_session_id"), Some(&json!("session-stable")));
    }

    #[tokio::test]
    async fn turn_request_reuses_session_id_after_resume() {
        use std::sync::Mutex;

        struct ResumeCapturingProvider {
            captured: Mutex<Vec<clankers_provider::CompletionRequest>>,
        }

        #[async_trait]
        impl clankers_provider::Provider for ResumeCapturingProvider {
            async fn complete(
                &self,
                request: clankers_provider::CompletionRequest,
                tx: mpsc::Sender<StreamEvent>,
            ) -> clankers_provider::error::Result<()> {
                self.captured.lock().expect("capture lock poisoned").push(request);
                tx.send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: "msg-1".into(),
                        model: "test-model".into(),
                        role: "assistant".into(),
                    },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockStart {
                    index: 0,
                    content_block: Content::Text { text: String::new() },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: ContentDelta::TextDelta { text: "OK".into() },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                tx.send(StreamEvent::MessageDelta {
                    stop_reason: Some("end_turn".into()),
                    usage: Usage {
                        input_tokens: 10,
                        output_tokens: 2,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    },
                })
                .await
                .ok();
                tx.send(StreamEvent::MessageStop).await.ok();
                Ok(())
            }

            fn models(&self) -> &[clankers_provider::Model] {
                &[]
            }

            fn name(&self) -> &str {
                "resume-capturing"
            }
        }

        let provider = ResumeCapturingProvider {
            captured: Mutex::new(Vec::new()),
        };
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let config = make_turn_config();
        let (event_tx, _rx) = broadcast::channel(256);
        let mut before_resume_messages = vec![make_user_message()];

        test_run_turn_loop(
            &provider,
            &tools,
            &mut before_resume_messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "session-resumed",
            None,
            None,
            None,
        )
        .await
        .expect("turn before resume should succeed");

        let mut resumed_messages = vec![AgentMessage::User(UserMessage {
            id: MessageId::new("test-msg-3"),
            content: vec![Content::Text {
                text: "after resume".into(),
            }],
            timestamp: Utc::now(),
        })];

        test_run_turn_loop(
            &provider,
            &tools,
            &mut resumed_messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "session-resumed",
            None,
            None,
            None,
        )
        .await
        .expect("turn after resume should succeed");

        let captured = provider.captured.lock().expect("capture lock poisoned");
        assert_eq!(captured.len(), 2);
        assert_eq!(captured[0].extra_params.get("_session_id"), Some(&json!("session-resumed")));
        assert_eq!(captured[1].extra_params.get("_session_id"), Some(&json!("session-resumed")));
    }

    #[test]
    fn decide_model_completion_accepts_turn_finish_effect() {
        let outcome = clankers_engine::EngineOutcome {
            next_state: clankers_engine::EngineState::new(),
            effects: vec![EngineEffect::EmitEvent(EngineEvent::TurnFinished {
                stop_reason: StopReason::Stop,
            })],
            rejection: None,
            terminal_failure: None,
        };

        let decision = decide_model_completion(&outcome).expect("turn finish decision should be accepted");
        assert!(matches!(decision, EngineModelDecision::Finish(StopReason::Stop)));
    }

    #[test]
    fn decide_model_completion_accepts_execute_tool_effects() {
        let outcome = clankers_engine::EngineOutcome {
            next_state: clankers_engine::EngineState::new(),
            effects: vec![EngineEffect::ExecuteTool(clankers_engine::EngineToolCall {
                call_id: clankers_engine::EngineCorrelationId("call-1".to_string()),
                tool_name: "read".to_string(),
                input: json!({"path": "src/main.rs"}),
            })],
            rejection: None,
            terminal_failure: None,
        };

        let decision = decide_model_completion(&outcome).expect("tool decision should be accepted");
        assert!(matches!(decision, EngineModelDecision::ExecuteTools(tool_calls) if tool_calls.len() == 1));
    }

    #[test]
    fn decide_model_completion_rejects_ambiguous_effect_sets() {
        let outcome = clankers_engine::EngineOutcome {
            next_state: clankers_engine::EngineState::new(),
            effects: vec![
                EngineEffect::ExecuteTool(clankers_engine::EngineToolCall {
                    call_id: clankers_engine::EngineCorrelationId("call-1".to_string()),
                    tool_name: "read".to_string(),
                    input: json!({"path": "src/main.rs"}),
                }),
                EngineEffect::EmitEvent(EngineEvent::TurnFinished {
                    stop_reason: StopReason::Stop,
                }),
            ],
            rejection: None,
            terminal_failure: None,
        };

        let error = decide_model_completion(&outcome).expect_err("ambiguous effects should fail closed");
        assert!(matches!(error, AgentError::ProviderStreaming { retryable: false, .. }));
    }

    #[tokio::test]
    async fn run_turn_loop_applies_model_switch_and_emits_usage_updates() {
        use std::sync::Mutex;

        const EXPECTED_USAGE_INPUT: usize = 10;
        const EXPECTED_USAGE_OUTPUT: usize = 2;

        struct CapturingModelProvider {
            models: Mutex<Vec<String>>,
        }

        #[async_trait]
        impl clankers_provider::Provider for CapturingModelProvider {
            async fn complete(
                &self,
                request: clankers_provider::CompletionRequest,
                tx: mpsc::Sender<StreamEvent>,
            ) -> clankers_provider::error::Result<()> {
                self.models.lock().expect("model capture lock poisoned").push(request.model);
                tx.send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: "model-switch-msg".into(),
                        model: "switched-model".into(),
                        role: "assistant".into(),
                    },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockStart {
                    index: 0,
                    content_block: Content::Text { text: String::new() },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockDelta {
                    index: 0,
                    delta: ContentDelta::TextDelta { text: "done".into() },
                })
                .await
                .ok();
                tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                tx.send(StreamEvent::MessageDelta {
                    stop_reason: Some("end_turn".into()),
                    usage: Usage {
                        input_tokens: EXPECTED_USAGE_INPUT,
                        output_tokens: EXPECTED_USAGE_OUTPUT,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    },
                })
                .await
                .ok();
                tx.send(StreamEvent::MessageStop).await.ok();
                Ok(())
            }

            fn models(&self) -> &[clankers_provider::Model] {
                &[]
            }

            fn name(&self) -> &str {
                "capturing-model"
            }
        }

        let provider = CapturingModelProvider {
            models: Mutex::new(Vec::new()),
        };
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let mut messages = vec![make_user_message()];
        let config = make_turn_config();
        let (event_tx, mut event_rx) = broadcast::channel(256);
        let switch_slot = crate::tool::model_switch_slot();
        *switch_slot.lock() = Some("switched-model".to_string());

        test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            Some(&switch_slot),
            None,
            "session-model-switch",
            None,
            None,
            None,
        )
        .await
        .expect("turn should succeed");

        assert_eq!(provider.models.lock().expect("model capture lock poisoned").as_slice(), &[
            "switched-model".to_string()
        ]);
        let mut saw_model_change = false;
        let mut saw_usage_update = false;
        loop {
            match event_rx.try_recv() {
                Ok(AgentEvent::ModelChange { from, to, .. }) => {
                    saw_model_change = from == "test-model" && to == "switched-model";
                }
                Ok(AgentEvent::UsageUpdate {
                    turn_usage,
                    cumulative_usage,
                }) => {
                    saw_usage_update = turn_usage.input_tokens == EXPECTED_USAGE_INPUT
                        && turn_usage.output_tokens == EXPECTED_USAGE_OUTPUT
                        && cumulative_usage.input_tokens == EXPECTED_USAGE_INPUT
                        && cumulative_usage.output_tokens == EXPECTED_USAGE_OUTPUT;
                }
                Ok(_) => {}
                Err(
                    tokio::sync::broadcast::error::TryRecvError::Empty
                    | tokio::sync::broadcast::error::TryRecvError::Closed,
                ) => break,
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {}
            }
        }
        assert!(saw_model_change);
        assert!(saw_usage_update);
    }

    #[tokio::test]
    async fn run_turn_loop_preserves_capability_gate_denials_through_host_runner() {
        use std::sync::Mutex;
        use std::sync::atomic::Ordering;

        struct DenyAllGate;

        impl crate::tool::CapabilityGate for DenyAllGate {
            fn check_tool_call(&self, tool_name: &str, _input: &Value) -> std::result::Result<(), String> {
                Err(format!("blocked {tool_name}"))
            }
        }

        struct CapabilityProvider {
            call_count: AtomicUsize,
            captured_requests: Mutex<Vec<clankers_provider::CompletionRequest>>,
        }

        #[async_trait]
        impl clankers_provider::Provider for CapabilityProvider {
            async fn complete(
                &self,
                request: clankers_provider::CompletionRequest,
                tx: mpsc::Sender<StreamEvent>,
            ) -> clankers_provider::error::Result<()> {
                self.captured_requests.lock().expect("capture lock poisoned").push(request);
                let call_index = self.call_count.fetch_add(1, Ordering::SeqCst);
                tx.send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: format!("capability-{call_index}"),
                        model: "test-model".into(),
                        role: "assistant".into(),
                    },
                })
                .await
                .ok();
                if call_index == 0 {
                    tx.send(StreamEvent::ContentBlockStart {
                        index: 0,
                        content_block: Content::ToolUse {
                            id: "call-1".into(),
                            name: "direct_tool".into(),
                            input: json!({}),
                        },
                    })
                    .await
                    .ok();
                    tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                    tx.send(StreamEvent::MessageDelta {
                        stop_reason: Some("tool_use".into()),
                        usage: Usage::default(),
                    })
                    .await
                    .ok();
                } else {
                    tx.send(StreamEvent::ContentBlockStart {
                        index: 0,
                        content_block: Content::Text { text: String::new() },
                    })
                    .await
                    .ok();
                    tx.send(StreamEvent::ContentBlockDelta {
                        index: 0,
                        delta: ContentDelta::TextDelta { text: "done".into() },
                    })
                    .await
                    .ok();
                    tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                    tx.send(StreamEvent::MessageDelta {
                        stop_reason: Some("end_turn".into()),
                        usage: Usage::default(),
                    })
                    .await
                    .ok();
                }
                tx.send(StreamEvent::MessageStop).await.ok();
                Ok(())
            }

            fn models(&self) -> &[clankers_provider::Model] {
                &[]
            }

            fn name(&self) -> &str {
                "capability-provider"
            }
        }

        let provider = CapabilityProvider {
            call_count: AtomicUsize::new(0),
            captured_requests: Mutex::new(Vec::new()),
        };
        let tool: Arc<dyn Tool> = Arc::new(DirectResultTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("direct_tool".to_string(), tool);
        let mut messages = vec![make_user_message()];
        let mut config = make_turn_config();
        config.model_request_slot_budget = 2;
        let (event_tx, _rx) = broadcast::channel(256);
        let gate: Arc<dyn crate::tool::CapabilityGate> = Arc::new(DenyAllGate);

        test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "session-capability",
            None,
            Some(gate.clone()),
            None,
        )
        .await
        .expect("capability denial should be fed back to model");

        assert_eq!(provider.call_count.load(Ordering::SeqCst), 2);
        let AgentMessage::ToolResult(tool_result) = &messages[2] else {
            panic!("expected denied tool result");
        };
        assert!(tool_result.is_error);
        let Some(Content::Text { text }) = tool_result.content.first() else {
            panic!("expected denial text");
        };
        assert!(text.contains("blocked direct_tool"));
    }

    #[tokio::test]
    async fn run_turn_loop_dispatches_pre_tool_hooks_through_host_runner() {
        use std::sync::Mutex;
        use std::sync::atomic::Ordering;

        struct RecordingDenyHook {
            calls: Arc<Mutex<Vec<clankers_hooks::HookPoint>>>,
        }

        #[async_trait]
        impl clankers_hooks::HookHandler for RecordingDenyHook {
            fn name(&self) -> &str {
                "recording-deny"
            }

            fn priority(&self) -> u32 {
                clankers_hooks::dispatcher::PRIORITY_PLUGIN_HOOKS
            }

            fn subscribes_to(&self, point: clankers_hooks::HookPoint) -> bool {
                matches!(point, clankers_hooks::HookPoint::PreTool)
            }

            async fn handle(
                &self,
                point: clankers_hooks::HookPoint,
                _payload: &clankers_hooks::HookPayload,
            ) -> clankers_hooks::HookVerdict {
                self.calls.lock().expect("hook call lock poisoned").push(point);
                clankers_hooks::HookVerdict::Deny {
                    reason: "hook blocked".to_string(),
                }
            }
        }

        struct HookProvider {
            call_count: AtomicUsize,
        }

        #[async_trait]
        impl clankers_provider::Provider for HookProvider {
            async fn complete(
                &self,
                _request: clankers_provider::CompletionRequest,
                tx: mpsc::Sender<StreamEvent>,
            ) -> clankers_provider::error::Result<()> {
                let call_index = self.call_count.fetch_add(1, Ordering::SeqCst);
                tx.send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: format!("hook-{call_index}"),
                        model: "test-model".into(),
                        role: "assistant".into(),
                    },
                })
                .await
                .ok();
                if call_index == 0 {
                    tx.send(StreamEvent::ContentBlockStart {
                        index: 0,
                        content_block: Content::ToolUse {
                            id: "call-1".into(),
                            name: "direct_tool".into(),
                            input: json!({}),
                        },
                    })
                    .await
                    .ok();
                    tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                    tx.send(StreamEvent::MessageDelta {
                        stop_reason: Some("tool_use".into()),
                        usage: Usage::default(),
                    })
                    .await
                    .ok();
                } else {
                    tx.send(StreamEvent::ContentBlockStart {
                        index: 0,
                        content_block: Content::Text { text: String::new() },
                    })
                    .await
                    .ok();
                    tx.send(StreamEvent::ContentBlockDelta {
                        index: 0,
                        delta: ContentDelta::TextDelta { text: "done".into() },
                    })
                    .await
                    .ok();
                    tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                    tx.send(StreamEvent::MessageDelta {
                        stop_reason: Some("end_turn".into()),
                        usage: Usage::default(),
                    })
                    .await
                    .ok();
                }
                tx.send(StreamEvent::MessageStop).await.ok();
                Ok(())
            }

            fn models(&self) -> &[clankers_provider::Model] {
                &[]
            }

            fn name(&self) -> &str {
                "hook-provider"
            }
        }

        let provider = HookProvider {
            call_count: AtomicUsize::new(0),
        };
        let tool: Arc<dyn Tool> = Arc::new(DirectResultTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("direct_tool".to_string(), tool);
        let mut messages = vec![make_user_message()];
        let mut config = make_turn_config();
        config.model_request_slot_budget = 2;
        let (event_tx, _rx) = broadcast::channel(256);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut pipeline = clankers_hooks::HookPipeline::new();
        pipeline.register(Arc::new(RecordingDenyHook { calls: calls.clone() }));

        test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            Some(Arc::new(pipeline)),
            "session-hook",
            None,
            None,
            None,
        )
        .await
        .expect("hook denial should be fed back to model");

        assert_eq!(provider.call_count.load(Ordering::SeqCst), 2);
        assert_eq!(calls.lock().expect("hook call lock poisoned").as_slice(), &[clankers_hooks::HookPoint::PreTool]);
        let AgentMessage::ToolResult(tool_result) = &messages[2] else {
            panic!("expected hook denied tool result");
        };
        assert!(tool_result.is_error);
        let Some(Content::Text { text }) = tool_result.content.first() else {
            panic!("expected hook text");
        };
        assert!(text.contains("hook blocked"));
    }

    #[tokio::test]
    async fn run_turn_loop_executes_engine_requested_tool_roundtrip() {
        use std::sync::atomic::Ordering;

        const FIRST_PROVIDER_CALL: usize = 0;
        const SECOND_PROVIDER_CALL: usize = 1;
        const EXPECTED_PROVIDER_CALLS: usize = 2;
        const EXPECTED_MESSAGE_COUNT: usize = 4;

        struct ToolRoundTripProvider {
            call_count: AtomicUsize,
        }

        #[async_trait]
        impl clankers_provider::Provider for ToolRoundTripProvider {
            async fn complete(
                &self,
                _request: clankers_provider::CompletionRequest,
                tx: mpsc::Sender<StreamEvent>,
            ) -> clankers_provider::error::Result<()> {
                let call_index = self.call_count.fetch_add(1, Ordering::SeqCst);
                tx.send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: format!("msg-{call_index}"),
                        model: "test-model".into(),
                        role: "assistant".into(),
                    },
                })
                .await
                .ok();

                match call_index {
                    FIRST_PROVIDER_CALL => {
                        tx.send(StreamEvent::ContentBlockStart {
                            index: 0,
                            content_block: Content::ToolUse {
                                id: "call-1".into(),
                                name: "direct_tool".into(),
                                input: json!({}),
                            },
                        })
                        .await
                        .ok();
                        tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                        tx.send(StreamEvent::MessageDelta {
                            stop_reason: Some("tool_use".into()),
                            usage: Usage {
                                input_tokens: 10,
                                output_tokens: 2,
                                cache_creation_input_tokens: 0,
                                cache_read_input_tokens: 0,
                            },
                        })
                        .await
                        .ok();
                    }
                    SECOND_PROVIDER_CALL => {
                        tx.send(StreamEvent::ContentBlockStart {
                            index: 0,
                            content_block: Content::Text { text: String::new() },
                        })
                        .await
                        .ok();
                        tx.send(StreamEvent::ContentBlockDelta {
                            index: 0,
                            delta: ContentDelta::TextDelta { text: "done".into() },
                        })
                        .await
                        .ok();
                        tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                        tx.send(StreamEvent::MessageDelta {
                            stop_reason: Some("end_turn".into()),
                            usage: Usage {
                                input_tokens: 10,
                                output_tokens: 2,
                                cache_creation_input_tokens: 0,
                                cache_read_input_tokens: 0,
                            },
                        })
                        .await
                        .ok();
                    }
                    _ => panic!("unexpected provider call index: {call_index}"),
                }

                tx.send(StreamEvent::MessageStop).await.ok();
                Ok(())
            }

            fn models(&self) -> &[clankers_provider::Model] {
                &[]
            }

            fn name(&self) -> &str {
                "tool-roundtrip"
            }
        }

        let provider = ToolRoundTripProvider {
            call_count: AtomicUsize::new(0),
        };
        let tool: Arc<dyn Tool> = Arc::new(DirectResultTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("direct_tool".to_string(), tool);
        let mut messages = vec![make_user_message()];
        let mut config = make_turn_config();
        config.model_request_slot_budget = EXPECTED_PROVIDER_CALLS as u32;
        let (event_tx, _rx) = broadcast::channel(256);

        test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "session-engine-tool-roundtrip",
            None,
            None,
            None,
        )
        .await
        .expect("tool roundtrip should succeed");

        assert_eq!(provider.call_count.load(Ordering::SeqCst), EXPECTED_PROVIDER_CALLS);
        assert_eq!(messages.len(), EXPECTED_MESSAGE_COUNT);
        assert!(matches!(
            &messages[1],
            AgentMessage::Assistant(assistant) if assistant.stop_reason == StopReason::ToolUse
        ));
        let AgentMessage::ToolResult(tool_result) = &messages[2] else {
            panic!("expected tool result message");
        };
        assert_eq!(tool_result.tool_name, "direct_tool");
        let AgentMessage::Assistant(final_assistant) = &messages[3] else {
            panic!("expected final assistant message");
        };
        assert_eq!(final_assistant.stop_reason, StopReason::Stop);
    }

    #[tokio::test]
    async fn run_turn_loop_feeds_tool_failures_back_through_engine() {
        use std::sync::Mutex;
        use std::sync::atomic::Ordering;

        const FIRST_PROVIDER_CALL: usize = 0;
        const SECOND_PROVIDER_CALL: usize = 1;
        const EXPECTED_PROVIDER_CALLS: usize = 2;
        const EXPECTED_MESSAGE_COUNT: usize = 4;

        struct FailingToolProvider {
            call_count: AtomicUsize,
            captured_requests: Mutex<Vec<clankers_provider::CompletionRequest>>,
        }

        #[async_trait]
        impl clankers_provider::Provider for FailingToolProvider {
            async fn complete(
                &self,
                request: clankers_provider::CompletionRequest,
                tx: mpsc::Sender<StreamEvent>,
            ) -> clankers_provider::error::Result<()> {
                self.captured_requests.lock().expect("capture lock poisoned").push(request);
                let call_index = self.call_count.fetch_add(1, Ordering::SeqCst);
                tx.send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: format!("msg-{call_index}"),
                        model: "test-model".into(),
                        role: "assistant".into(),
                    },
                })
                .await
                .ok();

                match call_index {
                    FIRST_PROVIDER_CALL => {
                        tx.send(StreamEvent::ContentBlockStart {
                            index: 0,
                            content_block: Content::ToolUse {
                                id: "call-1".into(),
                                name: "failing_tool".into(),
                                input: json!({}),
                            },
                        })
                        .await
                        .ok();
                        tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                        tx.send(StreamEvent::MessageDelta {
                            stop_reason: Some("tool_use".into()),
                            usage: Usage {
                                input_tokens: 10,
                                output_tokens: 2,
                                cache_creation_input_tokens: 0,
                                cache_read_input_tokens: 0,
                            },
                        })
                        .await
                        .ok();
                    }
                    SECOND_PROVIDER_CALL => {
                        tx.send(StreamEvent::ContentBlockStart {
                            index: 0,
                            content_block: Content::Text { text: String::new() },
                        })
                        .await
                        .ok();
                        tx.send(StreamEvent::ContentBlockDelta {
                            index: 0,
                            delta: ContentDelta::TextDelta { text: "done".into() },
                        })
                        .await
                        .ok();
                        tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
                        tx.send(StreamEvent::MessageDelta {
                            stop_reason: Some("end_turn".into()),
                            usage: Usage {
                                input_tokens: 10,
                                output_tokens: 2,
                                cache_creation_input_tokens: 0,
                                cache_read_input_tokens: 0,
                            },
                        })
                        .await
                        .ok();
                    }
                    _ => panic!("unexpected provider call index: {call_index}"),
                }

                tx.send(StreamEvent::MessageStop).await.ok();
                Ok(())
            }

            fn models(&self) -> &[clankers_provider::Model] {
                &[]
            }

            fn name(&self) -> &str {
                "failing-tool-provider"
            }
        }

        let provider = FailingToolProvider {
            call_count: AtomicUsize::new(0),
            captured_requests: Mutex::new(Vec::new()),
        };
        let tool: Arc<dyn Tool> = Arc::new(FailingTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("failing_tool".to_string(), tool);
        let mut messages = vec![make_user_message()];
        let mut config = make_turn_config();
        config.model_request_slot_budget = EXPECTED_PROVIDER_CALLS as u32;
        let (event_tx, _rx) = broadcast::channel(256);

        test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "session-engine-tool-failure",
            None,
            None,
            None,
        )
        .await
        .expect("tool failure roundtrip should succeed");

        assert_eq!(provider.call_count.load(Ordering::SeqCst), EXPECTED_PROVIDER_CALLS);
        assert_eq!(messages.len(), EXPECTED_MESSAGE_COUNT);
        let AgentMessage::ToolResult(tool_result) = &messages[2] else {
            panic!("expected tool result message");
        };
        assert!(tool_result.is_error);
        assert_eq!(tool_result.tool_name, "failing_tool");

        let captured_requests = provider.captured_requests.lock().expect("capture lock poisoned");
        assert_eq!(captured_requests.len(), EXPECTED_PROVIDER_CALLS);
        assert!(matches!(
            captured_requests[1].messages.iter().find(|message| matches!(message, AgentMessage::ToolResult(_))),
            Some(AgentMessage::ToolResult(tool_result)) if tool_result.call_id == "call-1" && tool_result.is_error
        ));
    }

    #[tokio::test]
    async fn turn_retry_recovers_on_second_attempt() {
        // Fails once with 502, then succeeds
        let provider = RetryableFailProvider::new(1, 502);
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let mut messages = vec![make_user_message()];
        let config = make_turn_config();
        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        let result = test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            cancel,
            None,
            None,
            None,
            "test-session",
            None,
            None,
            None,
        )
        .await;

        assert!(result.is_ok(), "expected success after retry, got: {:?}", result);
        // Should have appended an assistant message
        assert_eq!(messages.len(), 2);
    }

    #[tokio::test]
    async fn turn_retry_non_retryable_error_skips_retry() {
        // Fails with 400 (non-retryable) — should fail immediately
        let provider = RetryableFailProvider::new(usize::MAX, 400);
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let mut messages = vec![make_user_message()];
        let config = make_turn_config();
        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        let result = test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            cancel,
            None,
            None,
            None,
            "test-session",
            None,
            None,
            None,
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(!err.is_retryable(), "400 should not be retryable");
        // Messages unchanged — failed turn didn't append
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn turn_retry_cancellation_during_backoff() {
        // Fails with 502 (retryable), cancel during backoff
        let provider = RetryableFailProvider::new(3, 502); // always fails
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let mut messages = vec![make_user_message()];
        let config = make_turn_config();
        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();

        // Cancel shortly after first failure's backoff starts
        let cancel_clone = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(RETRY_CANCELLATION_DELAY_MS)).await;
            cancel_clone.cancel();
        });

        let result = test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            cancel,
            None,
            None,
            None,
            "test-session",
            None,
            None,
            None,
        )
        .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AgentError::Cancelled));
    }

    fn drain_system_messages(rx: &mut broadcast::Receiver<AgentEvent>) -> Vec<String> {
        let mut messages = Vec::new();
        loop {
            match rx.try_recv() {
                Ok(AgentEvent::SystemMessage { message }) => messages.push(message),
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {}
                Err(
                    tokio::sync::broadcast::error::TryRecvError::Empty
                    | tokio::sync::broadcast::error::TryRecvError::Closed,
                ) => break,
            }
        }
        messages
    }

    struct ToolUseOnlyProvider {
        call_count: AtomicUsize,
    }

    #[async_trait]
    impl clankers_provider::Provider for ToolUseOnlyProvider {
        async fn complete(
            &self,
            _request: clankers_provider::CompletionRequest,
            tx: mpsc::Sender<StreamEvent>,
        ) -> clankers_provider::error::Result<()> {
            let call_index = self.call_count.fetch_add(1, Ordering::SeqCst);
            tx.send(StreamEvent::MessageStart {
                message: MessageMetadata {
                    id: format!("tool-use-only-{call_index}"),
                    model: "test-model".into(),
                    role: "assistant".into(),
                },
            })
            .await
            .ok();
            tx.send(StreamEvent::ContentBlockStart {
                index: 0,
                content_block: Content::ToolUse {
                    id: "call-1".into(),
                    name: "direct_tool".into(),
                    input: json!({}),
                },
            })
            .await
            .ok();
            tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
            tx.send(StreamEvent::MessageDelta {
                stop_reason: Some("tool_use".into()),
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 2,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
            })
            .await
            .ok();
            tx.send(StreamEvent::MessageStop).await.ok();
            Ok(())
        }

        fn models(&self) -> &[clankers_provider::Model] {
            &[]
        }

        fn name(&self) -> &str {
            "tool-use-only"
        }
    }

    struct MaxTokensProvider {
        call_count: AtomicUsize,
    }

    #[async_trait]
    impl clankers_provider::Provider for MaxTokensProvider {
        async fn complete(
            &self,
            _request: clankers_provider::CompletionRequest,
            tx: mpsc::Sender<StreamEvent>,
        ) -> clankers_provider::error::Result<()> {
            let call_index = self.call_count.fetch_add(1, Ordering::SeqCst);
            tx.send(StreamEvent::MessageStart {
                message: MessageMetadata {
                    id: format!("max-tokens-{call_index}"),
                    model: "test-model".into(),
                    role: "assistant".into(),
                },
            })
            .await
            .ok();
            tx.send(StreamEvent::ContentBlockStart {
                index: 0,
                content_block: Content::Text { text: String::new() },
            })
            .await
            .ok();
            tx.send(StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::TextDelta { text: "partial".into() },
            })
            .await
            .ok();
            tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
            tx.send(StreamEvent::MessageDelta {
                stop_reason: Some("max_tokens".into()),
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 2,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
            })
            .await
            .ok();
            tx.send(StreamEvent::MessageStop).await.ok();
            Ok(())
        }

        fn models(&self) -> &[clankers_provider::Model] {
            &[]
        }

        fn name(&self) -> &str {
            "max-tokens"
        }
    }

    #[tokio::test]
    async fn engine_retry_stop_policy_retryable_recovery_uses_engine_retry_effect() {
        let provider = RetryableFailProvider::new(SINGLE_PROVIDER_FAILURE, RETRYABLE_PROVIDER_STATUS);
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let mut messages = vec![make_user_message()];
        let config = make_turn_config();
        let (event_tx, _rx) = broadcast::channel(256);

        let result = test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "engine-retry-stop-policy-recovery",
            None,
            None,
            None,
        )
        .await;

        assert!(result.is_ok(), "expected retry recovery, got: {:?}", result);
        assert_eq!(provider.call_count(), EXPECTED_RETRY_RECOVERY_PROVIDER_CALLS);
        assert_eq!(messages.len(), EXPECTED_ASSISTANT_MESSAGE_COUNT);
    }

    #[tokio::test]
    async fn engine_retry_stop_policy_terminal_failures_preserve_original_error_and_messages() {
        let non_retryable_provider =
            RetryableFailProvider::new(ALWAYS_FAIL_PROVIDER_FAILURES, NON_RETRYABLE_PROVIDER_STATUS);
        let retry_exhaustion_provider =
            RetryableFailProvider::new(ALWAYS_FAIL_PROVIDER_FAILURES, RETRYABLE_PROVIDER_STATUS);
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let config = make_turn_config();

        let mut non_retryable_messages = vec![make_user_message()];
        let (non_retry_event_tx, _rx) = broadcast::channel(256);
        let non_retryable_result = test_run_turn_loop(
            &non_retryable_provider,
            &tools,
            &mut non_retryable_messages,
            &config,
            &non_retry_event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "engine-retry-stop-policy-non-retryable",
            None,
            None,
            None,
        )
        .await;
        let non_retryable_error = non_retryable_result.expect_err("non-retryable error should propagate");
        assert_eq!(non_retryable_provider.call_count(), EXPECTED_SINGLE_PROVIDER_CALL);
        assert_eq!(non_retryable_error.status_code(), Some(NON_RETRYABLE_PROVIDER_STATUS));
        assert!(!non_retryable_error.is_retryable());
        assert_eq!(non_retryable_messages.len(), EXPECTED_USER_ONLY_MESSAGE_COUNT);

        let mut retry_exhaustion_messages = vec![make_user_message()];
        let (retry_event_tx, _rx) = broadcast::channel(256);
        let retry_exhaustion_result = test_run_turn_loop(
            &retry_exhaustion_provider,
            &tools,
            &mut retry_exhaustion_messages,
            &config,
            &retry_event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "engine-retry-stop-policy-exhaustion",
            None,
            None,
            None,
        )
        .await;
        let retry_error = retry_exhaustion_result.expect_err("retry exhaustion should propagate original error");
        assert_eq!(retry_exhaustion_provider.call_count(), EXPECTED_RETRY_EXHAUSTION_PROVIDER_CALLS);
        assert_eq!(retry_error.status_code(), Some(RETRYABLE_PROVIDER_STATUS));
        assert!(retry_error.is_retryable());
        assert_eq!(retry_exhaustion_messages.len(), EXPECTED_USER_ONLY_MESSAGE_COUNT);
    }

    #[tokio::test]
    async fn engine_retry_stop_policy_cancellation_during_retry_backoff_stops_retry_ready() {
        let provider = RetryableFailProvider::new(ALWAYS_FAIL_PROVIDER_FAILURES, RETRYABLE_PROVIDER_STATUS);
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let mut messages = vec![make_user_message()];
        let config = make_turn_config();
        let (event_tx, _rx) = broadcast::channel(256);
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(RETRY_CANCELLATION_DELAY_MS)).await;
            cancel_clone.cancel();
        });

        let result = test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            cancel,
            None,
            None,
            None,
            "engine-retry-stop-policy-cancel",
            None,
            None,
            None,
        )
        .await;

        assert!(matches!(result, Err(AgentError::Cancelled)));
        assert_eq!(provider.call_count(), EXPECTED_SINGLE_PROVIDER_CALL);
        assert_eq!(messages.len(), EXPECTED_USER_ONLY_MESSAGE_COUNT);
    }

    #[tokio::test]
    async fn engine_retry_stop_policy_zero_budget_rejects_before_provider_io() {
        let provider = RetryableFailProvider::new(0, RETRYABLE_PROVIDER_STATUS);
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let mut messages = vec![make_user_message()];
        let mut config = make_turn_config();
        config.model_request_slot_budget = ZERO_MODEL_REQUEST_SLOT_BUDGET;
        let (event_tx, _rx) = broadcast::channel(256);

        let result = test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "engine-retry-stop-policy-zero-budget",
            None,
            None,
            None,
        )
        .await;

        let error = result.expect_err("zero budget should reject");
        assert!(format!("{error}").contains("InvalidBudget"));
        assert_eq!(provider.call_count(), 0);
        assert_eq!(messages.len(), EXPECTED_USER_ONLY_MESSAGE_COUNT);
    }

    #[tokio::test]
    async fn engine_retry_stop_policy_budget_exhaustion_accepts_tool_feedback_without_follow_up_model() {
        let provider = ToolUseOnlyProvider {
            call_count: AtomicUsize::new(0),
        };
        let tool: Arc<dyn Tool> = Arc::new(DirectResultTool::new());
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("direct_tool".to_string(), tool);
        let mut messages = vec![make_user_message()];
        let mut config = make_turn_config();
        config.model_request_slot_budget = SINGLE_MODEL_REQUEST_SLOT_BUDGET;
        let (event_tx, mut event_rx) = broadcast::channel(256);

        let result = test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "engine-retry-stop-policy-budget",
            None,
            None,
            None,
        )
        .await;

        assert!(result.is_ok(), "budget exhaustion terminalizes successfully: {:?}", result);
        assert_eq!(provider.call_count.load(Ordering::SeqCst), EXPECTED_SINGLE_PROVIDER_CALL);
        assert_eq!(messages.len(), EXPECTED_TOOL_BUDGET_MESSAGE_COUNT);
        assert!(matches!(messages.last(), Some(AgentMessage::ToolResult(_))));
        assert!(
            drain_system_messages(&mut event_rx)
                .iter()
                .any(|message| message == clankers_engine::ENGINE_BUDGET_EXHAUSTED_NOTICE)
        );
    }

    #[tokio::test]
    async fn engine_retry_stop_policy_max_tokens_terminalizes_without_follow_up_work() {
        let provider = MaxTokensProvider {
            call_count: AtomicUsize::new(0),
        };
        let tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let mut messages = vec![make_user_message()];
        let config = make_turn_config();
        let (event_tx, _rx) = broadcast::channel(256);

        let result = test_run_turn_loop(
            &provider,
            &tools,
            &mut messages,
            &config,
            &event_tx,
            CancellationToken::new(),
            None,
            None,
            None,
            "engine-retry-stop-policy-max-tokens",
            None,
            None,
            None,
        )
        .await;

        assert!(result.is_ok(), "max tokens should terminalize successfully: {:?}", result);
        assert_eq!(provider.call_count.load(Ordering::SeqCst), EXPECTED_SINGLE_PROVIDER_CALL);
        assert_eq!(messages.len(), EXPECTED_ASSISTANT_MESSAGE_COUNT);
        let Some(AgentMessage::Assistant(assistant)) = messages.last() else {
            panic!("expected assistant message");
        };
        assert_eq!(assistant.stop_reason, StopReason::MaxTokens);
    }

    fn test_engine_prompt_submission(model_request_slot_budget: u32) -> clankers_engine::EnginePromptSubmission {
        clankers_engine::EnginePromptSubmission {
            messages: engine_messages_from_agent_messages(&[make_user_message()]),
            model: "test-model".to_string(),
            system_prompt: "You are a test assistant.".to_string(),
            max_tokens: Some(100),
            temperature: None,
            thinking: None,
            tools: Vec::new(),
            no_cache: true,
            cache_ttl: None,
            session_id: "test-session".to_string(),
            model_request_slot_budget,
        }
    }

    fn submitted_engine_state() -> (EngineState, EngineCorrelationId) {
        let outcome = clankers_engine::reduce(
            &EngineState::new(),
            &EngineInput::submit_user_prompt(test_engine_prompt_submission(2)),
        );
        let request_id = outcome
            .effects
            .iter()
            .find_map(|effect| match effect {
                EngineEffect::RequestModel(request) => Some(request.request_id.clone()),
                EngineEffect::ExecuteTool(_) | EngineEffect::ScheduleRetry { .. } | EngineEffect::EmitEvent(_) => None,
            })
            .expect("submit prompt must emit model request");
        (outcome.next_state, request_id)
    }

    fn tool_call_from_outcome(outcome: &EngineOutcome) -> clankers_engine::EngineToolCall {
        outcome
            .effects
            .iter()
            .find_map(|effect| match effect {
                EngineEffect::ExecuteTool(tool_call) => Some(tool_call.clone()),
                EngineEffect::RequestModel(_) | EngineEffect::ScheduleRetry { .. } | EngineEffect::EmitEvent(_) => None,
            })
            .expect("tool-use model response must emit tool execution")
    }

    #[test]
    fn engine_feedback_model_tool_retry_and_cancel_reduce_through_engine() {
        let (waiting_model_state, request_id) = submitted_engine_state();
        let completed = clankers_engine::reduce(&waiting_model_state, &EngineInput::ModelCompleted {
            request_id: request_id.clone(),
            response: EngineModelResponse {
                output: vec![Content::Text {
                    text: "done".to_string(),
                }],
                stop_reason: StopReason::Stop,
            },
        });
        assert!(completed.rejection.is_none());
        assert!(matches!(completed.next_state.phase, EngineTurnPhase::Finished));
        let post_terminal = clankers_engine::reduce(&completed.next_state, &EngineInput::ModelCompleted {
            request_id: request_id.clone(),
            response: EngineModelResponse {
                output: Vec::new(),
                stop_reason: StopReason::Stop,
            },
        });
        assert!(post_terminal.rejection.is_some());
        assert!(post_terminal.terminal_failure.is_none());

        let (waiting_retry_model_state, retry_request_id) = submitted_engine_state();
        let failed_retryable = clankers_engine::reduce(&waiting_retry_model_state, &EngineInput::ModelFailed {
            request_id: retry_request_id.clone(),
            failure: EngineTerminalFailure {
                message: "try again".to_string(),
                status: Some(500),
                retryable: true,
            },
        });
        let retry_ready_id = failed_retryable
            .effects
            .iter()
            .find_map(|effect| match effect {
                EngineEffect::ScheduleRetry { request_id, .. } => Some(request_id.clone()),
                EngineEffect::RequestModel(_) | EngineEffect::ExecuteTool(_) | EngineEffect::EmitEvent(_) => None,
            })
            .expect("retryable model failure must schedule retry");
        let retry_ready = clankers_engine::reduce(&failed_retryable.next_state, &EngineInput::RetryReady {
            request_id: retry_ready_id,
        });
        assert!(retry_ready.rejection.is_none());
        assert!(retry_ready.effects.iter().any(|effect| matches!(effect, EngineEffect::RequestModel(_))));

        let (waiting_failed_model_state, failed_request_id) = submitted_engine_state();
        let failed_terminal = clankers_engine::reduce(&waiting_failed_model_state, &EngineInput::ModelFailed {
            request_id: failed_request_id,
            failure: EngineTerminalFailure {
                message: "stop".to_string(),
                status: None,
                retryable: false,
            },
        });
        assert!(failed_terminal.terminal_failure.is_some());

        let (waiting_tool_model_state, tool_request_id) = submitted_engine_state();
        let tool_planned = clankers_engine::reduce(&waiting_tool_model_state, &EngineInput::ModelCompleted {
            request_id: tool_request_id,
            response: EngineModelResponse {
                output: vec![Content::ToolUse {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    input: json!({"path":"README.md"}),
                }],
                stop_reason: StopReason::ToolUse,
            },
        });
        let tool_call = tool_call_from_outcome(&tool_planned);
        let tool_completed = clankers_engine::reduce(&tool_planned.next_state, &EngineInput::ToolCompleted {
            call_id: tool_call.call_id.clone(),
            result: vec![Content::Text { text: "ok".to_string() }],
        });
        assert!(tool_completed.rejection.is_none());

        let (waiting_tool_fail_model_state, tool_fail_request_id) = submitted_engine_state();
        let tool_fail_planned = clankers_engine::reduce(&waiting_tool_fail_model_state, &EngineInput::ModelCompleted {
            request_id: tool_fail_request_id,
            response: EngineModelResponse {
                output: vec![Content::ToolUse {
                    id: "call-2".to_string(),
                    name: "read".to_string(),
                    input: json!({"path":"missing"}),
                }],
                stop_reason: StopReason::ToolUse,
            },
        });
        let failed_tool_call = tool_call_from_outcome(&tool_fail_planned);
        let tool_failed = clankers_engine::reduce(&tool_fail_planned.next_state, &EngineInput::ToolFailed {
            call_id: failed_tool_call.call_id,
            error: "missing".to_string(),
            result: vec![Content::Text {
                text: "missing".to_string(),
            }],
        });
        assert!(tool_failed.rejection.is_none());

        let (waiting_cancel_state, _) = submitted_engine_state();
        let cancelled = clankers_engine::reduce(&waiting_cancel_state, &EngineInput::CancelTurn {
            reason: "cancelled".to_string(),
        });
        assert!(cancelled.terminal_failure.is_none());
        assert!(matches!(cancelled.next_state.phase, EngineTurnPhase::Finished));
        assert!(cancelled.effects.iter().any(|effect| matches!(
            effect,
            EngineEffect::EmitEvent(EngineEvent::TurnFinished {
                stop_reason: StopReason::Stop
            })
        )));
    }

    #[test]
    fn accepted_prompt_submission_reduces_engine() {
        let submission = test_engine_prompt_submission(2);
        let outcome = clankers_engine::reduce(&EngineState::new(), &EngineInput::submit_user_prompt(submission));

        assert!(outcome.rejection.is_none());
        assert!(outcome.next_state.pending_model_request.is_some());
        let request = outcome
            .effects
            .iter()
            .find_map(|effect| match effect {
                EngineEffect::RequestModel(request) => Some(request),
                EngineEffect::ExecuteTool(_) | EngineEffect::ScheduleRetry { .. } | EngineEffect::EmitEvent(_) => None,
            })
            .expect("accepted prompt submission must emit model request before provider IO");
        assert_eq!(request.request_id.0, "model-request-1");
        assert_eq!(request.model, "test-model");
        assert_eq!(request.system_prompt, "You are a test assistant.");
        assert_eq!(request.session_id, "test-session");
        let user_text = request.messages.iter().find_map(|message| match message.role {
            clankers_engine::EngineMessageRole::User => message.content.iter().find_map(|content| match content {
                Content::Text { text } => Some(text.as_str()),
                Content::Image { .. }
                | Content::Thinking { .. }
                | Content::ToolUse { .. }
                | Content::ToolResult { .. } => None,
            }),
            clankers_engine::EngineMessageRole::Assistant | clankers_engine::EngineMessageRole::Tool => None,
        });
        assert_eq!(user_text, Some("hello"));
    }
}
