//! Steel-selected turn execution adapter.
//!
//! The Steel planner chooses whether a turn may use the reviewed Steel execution
//! seam. Provider/tool effects remain typed host calls: this adapter is the
//! explicit branch that runs those effects after Steel has authorized the turn.

#[cfg(test)]
use std::sync::atomic::AtomicUsize;
#[cfg(test)]
use std::sync::atomic::Ordering;

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
    run_engine_turn(seed, hosts).await
}
