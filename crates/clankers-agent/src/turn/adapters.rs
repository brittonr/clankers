use std::collections::HashMap;
use std::sync::Arc;

use clankers_engine::EngineCorrelationId;
use clankers_engine::EngineEvent;
use clankers_engine::EngineModelRequest;
use clankers_engine::EngineModelResponse;
use clankers_engine_host::CancellationSource;
use clankers_engine_host::EngineEventSink;
use clankers_engine_host::HostAdapterError;
use clankers_engine_host::ModelHost;
use clankers_engine_host::ModelHostOutcome;
use clankers_engine_host::RetrySleeper;
use clankers_engine_host::UsageObservation;
use clankers_engine_host::UsageObservationKind;
use clankers_engine_host::UsageObserver;
use clankers_model_selection::cost_tracker::CostTracker;
use clankers_provider::Provider;
use clankers_tool_host::ToolExecutor;
use clankers_tool_host::ToolHostOutcome;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use super::TurnTranscriptWriter;
use super::apply_output_truncation;
use super::build_assistant_message;
use super::check_model_switch;
use super::completion_request_from_engine_request;
use super::create_error_result;
use super::engine_failure_from_agent_error;
use super::execute_tools_parallel;
use super::stream_model_request;
use super::tool_result_message_to_host_outcome;
use super::tool_use_count;
use super::update_usage_tracking;
use crate::events::AgentEvent;
use crate::tool::ModelSwitchSlot;
use crate::tool::Tool;

const TURN_CANCELLED_REASON: &str = "turn cancelled";

pub(crate) struct AgentModelHost<'a> {
    pub(crate) provider: &'a dyn Provider,
    pub(crate) event_tx: &'a broadcast::Sender<AgentEvent>,
    pub(crate) cancel: CancellationToken,
    pub(crate) model_switch_slot: Option<&'a ModelSwitchSlot>,
    pub(crate) transcript: TurnTranscriptWriter,
}

impl ModelHost for AgentModelHost<'_> {
    async fn execute_model(&mut self, mut engine_request: EngineModelRequest) -> ModelHostOutcome {
        let mut active_model = self.transcript.active_model();
        if self.transcript.mark_turn_start(self.event_tx) {
            if let Err(error) = check_model_switch(&mut active_model, self.model_switch_slot, self.event_tx) {
                return ModelHostOutcome::Failed {
                    failure: engine_failure_from_agent_error(&error),
                };
            }
            self.transcript.set_active_model(active_model.clone());
        }
        engine_request.model = active_model;

        let request = match completion_request_from_engine_request(&engine_request) {
            Ok(r) => r,
            Err(error) => {
                return ModelHostOutcome::Failed {
                    failure: engine_failure_from_agent_error(&error),
                };
            }
        };

        match stream_model_request(self.provider, request, self.event_tx, &self.cancel).await {
            Ok(collected) => {
                let response = EngineModelResponse {
                    output: collected.content.clone(),
                    stop_reason: collected.stop_reason.clone(),
                };
                let usage = collected.usage.clone();
                let assistant = build_assistant_message(&collected);
                let tool_count = tool_use_count(&collected.content);
                self.transcript.append_assistant(assistant, tool_count);
                ModelHostOutcome::Completed {
                    response,
                    usage: Some(usage),
                }
            }
            Err(error) => ModelHostOutcome::Failed {
                failure: engine_failure_from_agent_error(&error),
            },
        }
    }
}

pub(crate) struct AgentToolHost<'a> {
    pub(crate) controller_tools: &'a HashMap<String, Arc<dyn Tool>>,
    pub(crate) event_tx: &'a broadcast::Sender<AgentEvent>,
    pub(crate) cancel: CancellationToken,
    pub(crate) hook_pipeline: Option<Arc<clankers_hooks::HookPipeline>>,
    pub(crate) session_id: &'a str,
    pub(crate) db: Option<clankers_db::Db>,
    pub(crate) capability_gate: Option<Arc<dyn crate::tool::CapabilityGate>>,
    pub(crate) user_tool_filter: Option<Vec<String>>,
    pub(crate) output_truncation: clanker_loop::OutputTruncationConfig,
    pub(crate) transcript: TurnTranscriptWriter,
}

impl ToolExecutor for AgentToolHost<'_> {
    async fn execute_tool(&mut self, call: clankers_engine::EngineToolCall) -> ToolHostOutcome {
        let call_id = call.call_id.0.clone();
        let tool_name = call.tool_name.clone();
        let tool_calls = vec![(call_id, tool_name, call.input)];
        let mut messages = execute_tools_parallel(
            self.controller_tools,
            &tool_calls,
            self.event_tx,
            self.cancel.clone(),
            self.hook_pipeline.clone(),
            self.session_id,
            self.db.clone(),
            self.capability_gate.clone(),
            self.user_tool_filter.clone(),
        )
        .await;
        let message = messages.pop().unwrap_or_else(|| {
            create_error_result(
                tool_calls[0].0.clone(),
                tool_calls[0].1.clone(),
                "tool host produced no result".to_string(),
                self.event_tx,
            )
        });
        let mut truncated = apply_output_truncation(vec![message], &self.output_truncation);
        let message = truncated.remove(0);
        let outcome = tool_result_message_to_host_outcome(&message);
        self.transcript.append_tool_result(message, self.event_tx);
        outcome
    }
}

pub(crate) struct AgentRetrySleeper {
    pub(crate) cancel: CancellationToken,
}

impl RetrySleeper for AgentRetrySleeper {
    async fn sleep_for_retry(
        &mut self,
        _request_id: EngineCorrelationId,
        delay: std::time::Duration,
    ) -> std::result::Result<(), HostAdapterError> {
        tokio::select! {
            () = self.cancel.cancelled() => Ok(()),
            () = tokio::time::sleep(delay) => Ok(()),
        }
    }
}

pub(crate) struct AgentEngineEventSink<'a> {
    pub(crate) event_tx: &'a broadcast::Sender<AgentEvent>,
    pub(crate) transcript: TurnTranscriptWriter,
}

impl EngineEventSink for AgentEngineEventSink<'_> {
    fn emit_engine_event(&mut self, event: &EngineEvent) -> std::result::Result<(), HostAdapterError> {
        if event.turn_finished_stop_reason().is_some() {
            self.transcript.finish_turn(self.event_tx);
            return Ok(());
        }

        match event {
            EngineEvent::Notice { message } => {
                self.event_tx
                    .send(AgentEvent::SystemMessage {
                        message: message.clone(),
                    })
                    .ok();
            }
            EngineEvent::BusyChanged { .. } => {}
            _ => {}
        }
        Ok(())
    }
}

pub(crate) struct AgentCancellationSource {
    pub(crate) cancel: CancellationToken,
}

impl CancellationSource for AgentCancellationSource {
    fn is_cancelled(&mut self) -> bool {
        self.cancel.is_cancelled()
    }

    fn cancellation_reason(&mut self) -> String {
        TURN_CANCELLED_REASON.to_string()
    }
}

pub(crate) struct AgentUsageObserver<'a> {
    pub(crate) cost_tracker: Option<&'a Arc<CostTracker>>,
    pub(crate) event_tx: &'a broadcast::Sender<AgentEvent>,
    pub(crate) transcript: TurnTranscriptWriter,
}

impl UsageObserver for AgentUsageObserver<'_> {
    fn observe_usage(&mut self, observation: &UsageObservation) -> std::result::Result<(), HostAdapterError> {
        if observation.kind != UsageObservationKind::FinalSummary {
            return Ok(());
        }
        let active_model = self.transcript.active_model();
        self.transcript.update_cumulative_usage(|cumulative| {
            update_usage_tracking(cumulative, &observation.usage, &active_model, self.cost_tracker, self.event_tx);
        });
        Ok(())
    }
}
