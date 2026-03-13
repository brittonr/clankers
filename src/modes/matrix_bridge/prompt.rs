//! Agent prompt execution for Matrix messages via actor sessions.

use std::sync::Arc;

use clankers_actor::ProcessRegistry;
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
) -> String {
    let (session_id, cmd_tx, event_tx) =
        get_or_create_keyed_session(&state, &registry, &factory, &key).await;

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
    let (_session_id, cmd_tx, event_tx) =
        get_or_create_keyed_session(&state, &registry, &factory, &key).await;

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
) -> String {
    let (session_id, cmd_tx, event_tx) =
        get_or_create_keyed_session(&state, &registry, &factory, &key).await;

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
