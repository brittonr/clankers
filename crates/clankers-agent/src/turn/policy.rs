#[cfg(test)]
use clankers_engine::EngineCorrelationId;
#[cfg(test)]
use clankers_engine::EngineEffect;
#[cfg(test)]
use clankers_engine::EngineEvent;
#[cfg(test)]
use clankers_engine::EngineInput;
#[cfg(test)]
use clankers_engine::EngineModelRequest;
use clankers_engine::EngineOutcome;
#[cfg(test)]
use clankers_engine::EngineState;
use clankers_engine::EngineTerminalFailure;
use clankers_engine_host::EngineRunReport;
#[cfg(test)]
use clankers_engine_host::runtime::tool_feedback_input as host_tool_feedback_input;
#[cfg(test)]
use clanker_message::StopReason;
#[cfg(test)]
use clanker_message::ToolResultMessage;
#[cfg(test)]
use serde_json::Value;
#[cfg(test)]
use tokio::sync::broadcast;

use crate::error::AgentError;
use crate::error::Result;
#[cfg(test)]
use crate::events::AgentEvent;

pub(crate) fn agent_error_from_report(report: &EngineRunReport) -> Option<AgentError> {
    if let Some(failure) = &report.last_outcome.terminal_failure {
        return Some(AgentError::ProviderStreaming {
            message: failure.message.clone(),
            status: failure.status,
            retryable: failure.retryable,
        });
    }
    report.last_outcome.rejection.as_ref().map(|rejection| AgentError::ProviderStreaming {
        message: format!("engine rejected turn: {rejection:?}"),
        status: None,
        retryable: false,
    })
}

#[derive(Debug)]
#[cfg(test)]
pub(crate) enum EngineModelDecision {
    ExecuteTools(Vec<(String, String, Value)>),
    Finish(StopReason),
}

#[cfg(test)]
#[allow(
    dead_code,
    reason = "kept as focused engine-effect helpers for decoupling regression tests"
)]
pub(crate) fn request_model_effect(outcome: &EngineOutcome) -> Result<EngineModelRequest> {
    let mut request_model = None;

    for effect in &outcome.effects {
        match effect {
            EngineEffect::RequestModel(model_request) => {
                if request_model.replace(model_request.clone()).is_some() {
                    return Err(AgentError::ProviderStreaming {
                        message: "engine emitted multiple model-request effects".to_string(),
                        status: None,
                        retryable: false,
                    });
                }
            }
            EngineEffect::ExecuteTool(_) | EngineEffect::ScheduleRetry { .. } | EngineEffect::EmitEvent(_) => {}
        }
    }

    request_model.ok_or_else(|| AgentError::ProviderStreaming {
        message: "engine omitted a required model-request effect".to_string(),
        status: None,
        retryable: false,
    })
}

#[cfg(test)]
#[allow(
    dead_code,
    reason = "kept as focused engine-effect helpers for decoupling regression tests"
)]
pub(crate) fn schedule_retry_effect(
    outcome: &EngineOutcome,
) -> Result<Option<(EngineCorrelationId, std::time::Duration)>> {
    let mut scheduled_retry = None;

    for effect in &outcome.effects {
        if let EngineEffect::ScheduleRetry { request_id, delay } = effect
            && scheduled_retry.replace((request_id.clone(), *delay)).is_some()
        {
            return Err(AgentError::ProviderStreaming {
                message: "engine emitted multiple retry-schedule effects".to_string(),
                status: None,
                retryable: false,
            });
        }
    }

    Ok(scheduled_retry)
}

pub(crate) fn engine_failure_from_agent_error(error: &AgentError) -> EngineTerminalFailure {
    EngineTerminalFailure {
        message: error.to_string(),
        status: error.status_code(),
        retryable: error.is_retryable(),
    }
}

#[cfg(test)]
pub(crate) fn decide_model_completion(outcome: &EngineOutcome) -> Result<EngineModelDecision> {
    let mut tool_calls = Vec::new();
    let mut turn_finished = None;

    for effect in &outcome.effects {
        match effect {
            EngineEffect::ExecuteTool(call) => {
                tool_calls.push((call.call_id.0.clone(), call.tool_name.clone(), call.input.clone()));
            }
            EngineEffect::RequestModel(_) | EngineEffect::ScheduleRetry { .. } | EngineEffect::EmitEvent(_) => {
                if let Some(stop_reason) = effect.turn_finished_stop_reason()
                    && turn_finished.replace(stop_reason.clone()).is_some()
                {
                    return Err(AgentError::ProviderStreaming {
                        message: "engine emitted multiple turn-finished effects".to_string(),
                        status: None,
                        retryable: false,
                    });
                }
            }
        }
    }

    let has_tool_calls = !tool_calls.is_empty();
    let has_turn_finish = turn_finished.is_some();
    if has_tool_calls == has_turn_finish {
        return Err(AgentError::ProviderStreaming {
            message: "engine emitted ambiguous model-completion effects".to_string(),
            status: None,
            retryable: false,
        });
    }

    match turn_finished {
        Some(stop_reason) => Ok(EngineModelDecision::Finish(stop_reason)),
        None => Ok(EngineModelDecision::ExecuteTools(tool_calls)),
    }
}

#[cfg(test)]
#[allow(
    dead_code,
    reason = "kept as focused engine-effect helpers for decoupling regression tests"
)]
pub(crate) fn emit_engine_notice_effects(outcome: &EngineOutcome, event_tx: &broadcast::Sender<AgentEvent>) {
    for effect in &outcome.effects {
        if let EngineEffect::EmitEvent(EngineEvent::Notice { message }) = effect {
            event_tx
                .send(AgentEvent::SystemMessage {
                    message: message.clone(),
                })
                .ok();
        }
    }
}

pub(crate) fn engine_outcome_or_error(engine_outcome: EngineOutcome, context: &str) -> Result<EngineOutcome> {
    if let Some(rejection) = &engine_outcome.rejection {
        return Err(AgentError::ProviderStreaming {
            message: format!("engine rejected {context}: {rejection:?}"),
            status: None,
            retryable: false,
        });
    }

    Ok(engine_outcome)
}

#[cfg(test)]
#[allow(
    dead_code,
    reason = "kept as focused engine-effect helpers for decoupling regression tests"
)]
pub(crate) fn update_engine_model(engine_state: &mut EngineState, active_model: &str) {
    if let Some(request_template) = engine_state.request_template.as_mut() {
        request_template.model = active_model.to_string();
    }
}

#[cfg(test)]
#[allow(
    dead_code,
    reason = "kept as focused engine-effect helpers for decoupling regression tests"
)]
pub(crate) fn tool_feedback_input(message: &ToolResultMessage) -> EngineInput {
    host_tool_feedback_input(
        clankers_engine::EngineCorrelationId(message.call_id.clone()),
        message.is_error,
        message.content.clone(),
    )
}
