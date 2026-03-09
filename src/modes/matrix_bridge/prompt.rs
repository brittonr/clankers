//! Agent prompt execution for Matrix messages.

use std::sync::Arc;

use clankers_auth::Capability;
use tokio::sync::RwLock;

use crate::agent::events::AgentEvent;
use crate::modes::daemon::SessionKey;
use crate::modes::daemon::SessionStore;
use crate::provider::message::Content;
use crate::provider::streaming::ContentDelta;

/// Run a prompt for a Matrix message and collect the full text response.
///
/// If `capabilities` is Some, the session is created with filtered tools.
/// If None, full access (allowlist user).
pub(crate) async fn run_matrix_prompt(
    store: Arc<RwLock<SessionStore>>,
    key: SessionKey,
    text: String,
    capabilities: Option<&[Capability]>,
) -> String {
    // Get conversation history
    let (mut agent, history) = {
        let mut store = store.write().await;
        let session = store.get_or_create(&key, capabilities);
        session.turn_count += 1;
        let messages = session.agent.messages().to_vec();
        let tools = session.session_tools.clone();
        let provider = Arc::clone(&store.provider);
        let settings = store.settings.clone();
        let model = store.model.clone();
        let system_prompt = store.system_prompt.clone();
        let agent = crate::agent::builder::AgentBuilder::new(provider, settings.clone(), model, system_prompt)
            .with_tools(tools)
            .build();
        (agent, messages)
    };

    agent.seed_messages(history);

    let mut rx = agent.subscribe();

    let collector = tokio::spawn(async move {
        let mut collected = String::new();
        while let Ok(event) = rx.recv().await {
            if let AgentEvent::MessageUpdate {
                delta: ContentDelta::TextDelta { ref text },
                ..
            } = event
            {
                collected.push_str(text);
            }
            if matches!(event, AgentEvent::AgentEnd { .. }) {
                break;
            }
        }
        collected
    });

    let result = agent.prompt(&text).await;
    let collected = collector.await.unwrap_or_default();

    // Save updated messages back
    let messages = agent.messages().to_vec();
    {
        let mut store = store.write().await;
        if let Some(session) = store.sessions.get_mut(&key) {
            session.agent.seed_messages(messages);
        }
    }

    match result {
        Ok(()) => collected,
        Err(e) => format!("Error: {e}"),
    }
}

/// Run a prompt with image content blocks (for vision models).
pub(crate) async fn run_matrix_prompt_with_images(
    store: Arc<RwLock<SessionStore>>,
    key: SessionKey,
    text: String,
    images: Vec<Content>,
    capabilities: Option<&[Capability]>,
) -> String {
    let (mut agent, history) = {
        let mut store = store.write().await;
        let session = store.get_or_create(&key, capabilities);
        session.turn_count += 1;
        let messages = session.agent.messages().to_vec();
        let tools = session.session_tools.clone();
        let provider = Arc::clone(&store.provider);
        let settings = store.settings.clone();
        let model = store.model.clone();
        let system_prompt = store.system_prompt.clone();
        let agent = crate::agent::builder::AgentBuilder::new(provider, settings.clone(), model, system_prompt)
            .with_tools(tools)
            .build();
        (agent, messages)
    };

    agent.seed_messages(history);

    let mut rx = agent.subscribe();

    let collector = tokio::spawn(async move {
        let mut collected = String::new();
        while let Ok(event) = rx.recv().await {
            if let AgentEvent::MessageUpdate {
                delta: ContentDelta::TextDelta { ref text },
                ..
            } = event
            {
                collected.push_str(text);
            }
            if matches!(event, AgentEvent::AgentEnd { .. }) {
                break;
            }
        }
        collected
    });

    let result = agent.prompt_with_images(&text, images).await;
    let collected = collector.await.unwrap_or_default();

    let messages = agent.messages().to_vec();
    {
        let mut store = store.write().await;
        if let Some(session) = store.sessions.get_mut(&key) {
            session.agent.seed_messages(messages);
        }
    }

    match result {
        Ok(()) => collected,
        Err(e) => format!("Error: {e}"),
    }
}

/// Run a prompt against a session without updating `last_active`.
/// Used for heartbeat and trigger prompts — these shouldn't prevent
/// idle reaping.
pub(crate) async fn run_proactive_prompt(store: Arc<RwLock<SessionStore>>, key: SessionKey, text: String) -> String {
    // Serialize via prompt lock
    let prompt_lock = {
        let mut store = store.write().await;
        store.prompt_lock(&key)
    };
    let _prompt_guard = prompt_lock.lock().await;

    let (mut agent, history) = {
        let mut store = store.write().await;
        let session = match store.sessions.get_mut(&key) {
            Some(s) => s,
            None => return String::new(), // session gone
        };
        // Deliberately do NOT update last_active or turn_count
        let messages = session.agent.messages().to_vec();
        let tools = session.session_tools.clone();
        let provider = Arc::clone(&store.provider);
        let settings = store.settings.clone();
        let model = store.model.clone();
        let system_prompt = store.system_prompt.clone();
        let agent = crate::agent::builder::AgentBuilder::new(provider, settings.clone(), model, system_prompt)
            .with_tools(tools)
            .build();
        (agent, messages)
    };

    agent.seed_messages(history);

    let mut rx = agent.subscribe();

    let collector = tokio::spawn(async move {
        let mut collected = String::new();
        while let Ok(event) = rx.recv().await {
            if let AgentEvent::MessageUpdate {
                delta: ContentDelta::TextDelta { ref text },
                ..
            } = event
            {
                collected.push_str(text);
            }
            if matches!(event, AgentEvent::AgentEnd { .. }) {
                break;
            }
        }
        collected
    });

    let result = agent.prompt(&text).await;
    let collected = collector.await.unwrap_or_default();

    // Save updated messages back
    let messages = agent.messages().to_vec();
    {
        let mut store = store.write().await;
        if let Some(session) = store.sessions.get_mut(&key) {
            session.agent.seed_messages(messages);
        }
    }

    match result {
        Ok(()) => collected,
        Err(e) => format!("Error: {e}"),
    }
}
