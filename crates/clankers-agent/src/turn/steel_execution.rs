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
use clankers_tool_host::ToolExecutor;
use tokio::sync::broadcast;

use crate::events::AgentEvent;

const STEEL_SELECTED_EXECUTION_RECEIPT_SCHEMA: &str = "clankers.steel_selected_execution.receipt.v1";
const STEEL_SELECTED_EXECUTION_SEAM: &str = "steel.host.execute_turn";
const STEEL_SELECTED_EXECUTION_RUNNER: &str = "RustHostRunner";
const STEEL_SELECTED_EXECUTION_EXECUTOR: &str = "SteelScheme";
const MAX_RECEIPT_TOKEN_CHARS: usize = 96;

pub(super) struct SteelSelectedExecutionReceiptContext<'a> {
    pub(super) session_id: &'a str,
    pub(super) model: &'a str,
    pub(super) event_tx: &'a broadcast::Sender<AgentEvent>,
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
) -> EngineRunReport
where
    M: ModelHost,
    T: ToolExecutor,
    R: RetrySleeper,
    E: EngineEventSink,
    C: CancellationSource,
    U: UsageObserver,
{
    #[cfg(test)]
    {
        STEEL_SELECTED_ENGINE_TURN_CALLS.fetch_add(1, Ordering::SeqCst);
    }
    let report = run_engine_turn(seed, hosts).await;
    emit_steel_selected_execution_receipt(&receipt_context, &report);
    report
}

fn emit_steel_selected_execution_receipt(context: &SteelSelectedExecutionReceiptContext<'_>, report: &EngineRunReport) {
    let session_hash = ArtifactHash::digest(context.session_id.as_bytes()).prefixed();
    let model = receipt_token(context.model);
    let status = execution_status(report);
    let observed_events = report.observed_events.len();
    let usage_observations = report.usage_observations.len();
    let diagnostics = report.adapter_diagnostics.len();
    let payload = format!(
        "schema={STEEL_SELECTED_EXECUTION_RECEIPT_SCHEMA}|seam={STEEL_SELECTED_EXECUTION_SEAM}|executor={STEEL_SELECTED_EXECUTION_EXECUTOR}|session_hash={session_hash}|model={model}|status={status}|host_runner={STEEL_SELECTED_EXECUTION_RUNNER}|observed_events={observed_events}|usage_observations={usage_observations}|diagnostics={diagnostics}",
    );
    let receipt_hash = ArtifactHash::digest(payload.as_bytes()).prefixed();
    let message = format!(
        "{STEEL_SELECTED_EXECUTION_SEAM} receipt schema={STEEL_SELECTED_EXECUTION_RECEIPT_SCHEMA} executor={STEEL_SELECTED_EXECUTION_EXECUTOR} session_hash={session_hash} model={model} status={status} host_runner={STEEL_SELECTED_EXECUTION_RUNNER} observed_events={observed_events} usage_observations={usage_observations} diagnostics={diagnostics} receipt_hash={receipt_hash}",
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

fn receipt_token(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars();
    for ch in chars.by_ref().take(MAX_RECEIPT_TOKEN_CHARS) {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/') {
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
