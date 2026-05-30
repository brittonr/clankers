//! Steel-selected turn execution adapter.
//!
//! The Steel planner chooses whether a turn may use the reviewed Steel execution
//! seam. Provider/tool effects remain typed host calls: this adapter is the
//! explicit branch that runs those effects after Steel has authorized the turn.

#[cfg(test)]
use std::sync::atomic::AtomicUsize;
#[cfg(test)]
use std::sync::atomic::Ordering;

use clankers_artifacts::ArtifactHash;
use clankers_engine_host::CancellationSource;
use clankers_engine_host::EngineEventSink;
use clankers_engine_host::EngineRunReport;
use clankers_engine_host::EngineRunSeed;
use clankers_engine_host::HostAdapters;
use clankers_engine_host::ModelHost;
use clankers_engine_host::RetrySleeper;
use clankers_engine_host::UsageObserver;
use clankers_engine_host::run_engine_turn;
use clankers_runtime::DEFAULT_TURN_EXECUTION_SEAM;
use clankers_runtime::DynamicRuntimeActionReason;
use clankers_runtime::DynamicRuntimeActionStatus;
use clankers_runtime::OrchestrationPlanReceipt;
use clankers_runtime::SteelOrchestrationProfile;
use clankers_runtime::SteelTurnExecutionInput;
use clankers_runtime::SteelTurnExecutionReceipt;
use clankers_runtime::SteelTurnPlanningAuthorityGrant;
use clankers_runtime::authorize_steel_turn_execution;
use clankers_tool_host::ToolExecutor;
use tokio::sync::broadcast;

use crate::error::AgentError;
use crate::error::Result;
use crate::events::AgentEvent;

const STEEL_SELECTED_EXECUTION_RECEIPT_SCHEMA: &str = "clankers.steel_selected_execution.receipt.v2";
const STEEL_SELECTED_EXECUTION_SEAM: &str = DEFAULT_TURN_EXECUTION_SEAM;
const STEEL_SELECTED_EXECUTION_RUNNER: &str = "RustHostRunner";
const STEEL_SELECTED_EXECUTION_EXECUTOR: &str = "SteelScheme";
const MAX_RECEIPT_TOKEN_CHARS: usize = 96;

pub(super) struct SteelSelectedExecutionReceiptContext<'a> {
    pub(super) session_id: &'a str,
    pub(super) model: &'a str,
    pub(super) event_tx: &'a broadcast::Sender<AgentEvent>,
    pub(super) profile: &'a SteelOrchestrationProfile,
    pub(super) planning_receipt: &'a OrchestrationPlanReceipt,
    pub(super) session_capabilities: &'a [String],
    pub(super) granted_ucan_abilities: &'a [String],
    pub(super) ucan_authority_grants: &'a [SteelTurnPlanningAuthorityGrant],
    pub(super) disabled_actions: &'a [String],
}

#[cfg(test)]
static STEEL_SELECTED_ENGINE_TURN_CALLS: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
pub(super) fn reset_steel_selected_engine_turn_call_count() {
    STEEL_SELECTED_ENGINE_TURN_CALLS.store(0, Ordering::SeqCst);
}

#[cfg(test)]
pub(super) fn steel_selected_engine_turn_call_count() -> usize {
    STEEL_SELECTED_ENGINE_TURN_CALLS.load(Ordering::SeqCst)
}

#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        tigerstyle::assertion_density,
        reason = "Steel-selected shell seam delegates typed host effects to the existing reducer-backed host runner"
    )
)]
pub(super) async fn run_steel_selected_engine_turn<M, T, R, E, C, U>(
    seed: EngineRunSeed,
    hosts: HostAdapters<'_, M, T, R, E, C, U>,
    receipt_context: SteelSelectedExecutionReceiptContext<'_>,
) -> Result<EngineRunReport>
where
    M: ModelHost,
    T: ToolExecutor,
    R: RetrySleeper,
    E: EngineEventSink,
    C: CancellationSource,
    U: UsageObserver,
{
    let execution_input = steel_turn_execution_input(&receipt_context);
    let authority = authorize_steel_turn_execution(receipt_context.profile, &execution_input);
    if !authority.is_allowed() {
        emit_steel_selected_execution_receipt(&receipt_context, &authority, None);
        return Err(AgentError::Agent {
            message: format!(
                "steel.host.execute_turn denied before provider request: {:?}",
                authority.authorization_receipt.reason
            ),
        });
    }
    #[cfg(test)]
    {
        STEEL_SELECTED_ENGINE_TURN_CALLS.fetch_add(1, Ordering::SeqCst);
    }
    let report = run_engine_turn(seed, hosts).await;
    emit_steel_selected_execution_receipt(&receipt_context, &authority, Some(&report));
    Ok(report)
}

fn steel_turn_execution_input(context: &SteelSelectedExecutionReceiptContext<'_>) -> SteelTurnExecutionInput {
    SteelTurnExecutionInput {
        turn_id: format!(
            "{}:{}",
            safe_route_token(context.session_id),
            context.planning_receipt.receipt_hash.prefixed()
        ),
        target_resource: format!("session:{}", safe_route_token(context.session_id)),
        plan_receipt_hash: context.planning_receipt.receipt_hash,
        plan_hash: context.planning_receipt.plan_hash,
        prompt_hash: context.planning_receipt.authorization_receipts.first().map_or_else(
            || ArtifactHash::digest(context.planning_receipt.receipt_hash.prefixed().as_bytes()),
            |receipt| receipt.input_hash,
        ),
        host_runner: STEEL_SELECTED_EXECUTION_RUNNER.to_string(),
        session_capabilities: context.session_capabilities.to_vec(),
        granted_ucan_abilities: context.granted_ucan_abilities.to_vec(),
        ucan_authority_grants: context.ucan_authority_grants.to_vec(),
        disabled_actions: context.disabled_actions.to_vec(),
    }
}

fn emit_steel_selected_execution_receipt(
    context: &SteelSelectedExecutionReceiptContext<'_>,
    authority: &SteelTurnExecutionReceipt,
    report: Option<&EngineRunReport>,
) {
    let session_hash = ArtifactHash::digest(context.session_id.as_bytes()).prefixed();
    let model = receipt_token(context.model);
    let status = report.map_or("Denied", execution_status);
    let observed_events = report.map_or(0, |report| report.observed_events.len());
    let usage_observations = report.map_or(0, |report| report.usage_observations.len());
    let diagnostics = report.map_or(0, |report| report.adapter_diagnostics.len());
    let authority_status = authority_status_label(authority.authorization_receipt.status);
    let authority_reason = authority_reason_label(authority.authorization_receipt.reason);
    let authority_receipt_hash = authority.authorization_receipt.receipt_hash.prefixed();
    let input_hash = authority.input_hash.prefixed();
    let input_bytes = authority.input_bytes;
    let receipt_hash = authority.receipt_hash.prefixed();
    let required_ucan = receipt_token(&authority.authorization_receipt.required_ucan_ability);
    let required_caps = receipt_token(&authority.authorization_receipt.required_session_capabilities.join(","));
    let message = format!(
        "{STEEL_SELECTED_EXECUTION_SEAM} receipt schema={STEEL_SELECTED_EXECUTION_RECEIPT_SCHEMA} executor={STEEL_SELECTED_EXECUTION_EXECUTOR} session_hash={session_hash} model={model} status={status} host_runner={STEEL_SELECTED_EXECUTION_RUNNER} authority_status={authority_status} authority_reason={authority_reason} required_ucan={required_ucan} required_caps={required_caps} input_hash={input_hash} input_bytes={input_bytes} authority_receipt_hash={authority_receipt_hash} observed_events={observed_events} usage_observations={usage_observations} diagnostics={diagnostics} receipt_hash={receipt_hash}",
    );
    context.event_tx.send(AgentEvent::SystemMessage { message }).ok();
}

fn execution_status(report: &EngineRunReport) -> &'static str {
    if report.last_outcome.terminal_failure.is_some() {
        return "TerminalFailure";
    }
    if report.last_outcome.rejection.is_some() {
        return "Rejected";
    }
    "Completed"
}

fn authority_status_label(status: DynamicRuntimeActionStatus) -> &'static str {
    match status {
        DynamicRuntimeActionStatus::Allowed => "Allowed",
        DynamicRuntimeActionStatus::PolicyDenied => "PolicyDenied",
        DynamicRuntimeActionStatus::UcanDenied => "UcanDenied",
        DynamicRuntimeActionStatus::Disabled => "Disabled",
        DynamicRuntimeActionStatus::InvalidEnvelope => "InvalidEnvelope",
    }
}

fn authority_reason_label(reason: DynamicRuntimeActionReason) -> &'static str {
    match reason {
        DynamicRuntimeActionReason::Ready => "Ready",
        DynamicRuntimeActionReason::InvalidSchema => "InvalidSchema",
        DynamicRuntimeActionReason::MissingRequiredField => "MissingRequiredField",
        DynamicRuntimeActionReason::UnsupportedRuntimeProfile => "UnsupportedRuntimeProfile",
        DynamicRuntimeActionReason::UnsupportedAction => "UnsupportedAction",
        DynamicRuntimeActionReason::DisabledAction => "DisabledAction",
        DynamicRuntimeActionReason::MissingSessionCapability => "MissingSessionCapability",
        DynamicRuntimeActionReason::MissingUcanAbility => "MissingUcanAbility",
        DynamicRuntimeActionReason::SecretBearingInput => "SecretBearingInput",
        DynamicRuntimeActionReason::InputTooLarge => "InputTooLarge",
        DynamicRuntimeActionReason::UnsafeReceiptDestination => "UnsafeReceiptDestination",
        DynamicRuntimeActionReason::UnsafeTargetResource => "UnsafeTargetResource",
    }
}

fn receipt_token(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars();
    for ch in chars.by_ref().take(MAX_RECEIPT_TOKEN_CHARS) {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/' | ',') {
            output.push(ch);
        } else {
            output.push('_');
        }
    }
    if chars.next().is_some() {
        output.push_str("_truncated");
    }
    output
}

fn safe_route_token(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}
