//! Agent prompt execution for Matrix messages via actor sessions.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::sync::Arc;

use clanker_actor::ProcessRegistry;
use clankers_controller::transport::DaemonState;
use clankers_protocol::SessionKey;
use tokio::sync::Mutex;

use crate::modes::daemon::agent_process::get_or_create_keyed_session;
use crate::modes::daemon::agent_process::prompt_and_collect;
use crate::modes::daemon::socket_bridge::SessionFactory;

/// Run a prompt for a Matrix message and collect the full text response.
pub(crate) async fn run_matrix_prompt(
    state: Arc<Mutex<DaemonState>>,
    registry: ProcessRegistry,
    factory: Arc<SessionFactory>,
    key: SessionKey,
    text: String,
    capabilities: Option<Vec<clankers_ucan::Capability>>,
) -> String {
    let (session_id, cmd_tx, event_tx) =
        get_or_create_keyed_session(&state, &registry, &factory, &key, capabilities).await;

    let response = prompt_and_collect(&cmd_tx, &event_tx, text, vec![]).await;

    // Update turn count and last_active
    {
        let mut st = state.lock().await;
        if let Some(h) = st.sessions.get_mut(&session_id) {
            h.turn_count += 1;
            h.last_active = chrono::Utc::now().to_rfc3339();
        }
    }

    response
}

/// Run a prompt without updating last_active (for heartbeats and triggers).
///
/// Idle reaping should not be prevented by proactive/automated prompts.
pub(crate) async fn run_proactive_prompt(
    state: Arc<Mutex<DaemonState>>,
    registry: ProcessRegistry,
    factory: Arc<SessionFactory>,
    key: SessionKey,
    text: String,
) -> String {
    let (_session_id, cmd_tx, event_tx) = get_or_create_keyed_session(&state, &registry, &factory, &key, None).await;

    prompt_and_collect(&cmd_tx, &event_tx, text, vec![]).await
}

/// Run a prompt with image content.
///
/// Images are passed as `ImageData` (base64) through the protocol. The
/// `SessionController` handles multi-content prompts via the agent.
pub(crate) async fn run_matrix_prompt_with_images(
    state: Arc<Mutex<DaemonState>>,
    registry: ProcessRegistry,
    factory: Arc<SessionFactory>,
    key: SessionKey,
    text: String,
    images: Vec<clankers_protocol::ImageData>,
    capabilities: Option<Vec<clankers_ucan::Capability>>,
) -> String {
    let (session_id, cmd_tx, event_tx) =
        get_or_create_keyed_session(&state, &registry, &factory, &key, capabilities).await;

    let response = prompt_and_collect(&cmd_tx, &event_tx, text, images).await;

    {
        let mut st = state.lock().await;
        if let Some(h) = st.sessions.get_mut(&session_id) {
            h.turn_count += 1;
            h.last_active = chrono::Utc::now().to_rfc3339();
        }
    }

    response
}
