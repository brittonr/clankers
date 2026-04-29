use std::sync::Arc;

use clankers_provider::Usage;
use clankers_provider::message::AgentMessage;
use clankers_provider::message::AssistantMessage;
use clankers_provider::message::ToolResultMessage;
use parking_lot::Mutex;
use tokio::sync::broadcast;

use crate::events::AgentEvent;

struct TranscriptInner {
    messages: Vec<AgentMessage>,
    cumulative_usage: Usage,
    active_model: String,
    turn_index: u32,
    turn_active: bool,
    pending_tool_count: usize,
    batch_tool_results: Vec<ToolResultMessage>,
    last_assistant: Option<AssistantMessage>,
}

pub(super) struct TurnTranscript {
    inner: Arc<Mutex<TranscriptInner>>,
}

impl TurnTranscript {
    pub(super) fn new(messages: Vec<AgentMessage>, initial_model: String) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TranscriptInner {
                messages,
                cumulative_usage: Usage::default(),
                active_model: initial_model,
                turn_index: 0,
                turn_active: false,
                pending_tool_count: 0,
                batch_tool_results: Vec::new(),
                last_assistant: None,
            })),
        }
    }

    pub(super) fn writer(&self) -> TurnTranscriptWriter {
        TurnTranscriptWriter {
            inner: self.inner.clone(),
        }
    }

    pub(super) fn into_messages(self) -> Vec<AgentMessage> {
        Arc::try_unwrap(self.inner)
            .unwrap_or_else(|arc| {
                let guard = arc.lock();
                Mutex::new(TranscriptInner {
                    messages: guard.messages.clone(),
                    cumulative_usage: guard.cumulative_usage.clone(),
                    active_model: guard.active_model.clone(),
                    turn_index: guard.turn_index,
                    turn_active: guard.turn_active,
                    pending_tool_count: guard.pending_tool_count,
                    batch_tool_results: guard.batch_tool_results.clone(),
                    last_assistant: guard.last_assistant.clone(),
                })
            })
            .into_inner()
            .messages
    }
}

#[derive(Clone)]
pub(super) struct TurnTranscriptWriter {
    inner: Arc<Mutex<TranscriptInner>>,
}

impl TurnTranscriptWriter {
    pub(super) fn append_assistant(&self, assistant: AssistantMessage, tool_count: usize) {
        let mut state = self.inner.lock();
        state.messages.push(AgentMessage::Assistant(assistant.clone()));
        state.last_assistant = Some(assistant);
        state.pending_tool_count = tool_count;
        state.batch_tool_results.clear();
    }

    pub(super) fn append_tool_result(&self, message: ToolResultMessage, event_tx: &broadcast::Sender<AgentEvent>) {
        let mut state = self.inner.lock();
        state.messages.push(AgentMessage::ToolResult(message.clone()));
        state.batch_tool_results.push(message);
        if state.pending_tool_count == 0 || state.batch_tool_results.len() < state.pending_tool_count {
            return;
        }

        let assistant = match state.last_assistant.clone() {
            Some(a) => a,
            None => return,
        };
        let tool_results = state.batch_tool_results.clone();
        state.batch_tool_results.clear();
        state.last_assistant = None;
        state.pending_tool_count = 0;
        state.turn_active = false;
        let turn_index = state.turn_index;
        state.turn_index = state.turn_index.saturating_add(1);
        drop(state);

        event_tx
            .send(AgentEvent::TurnEnd {
                index: turn_index,
                message: assistant,
                tool_results,
            })
            .ok();
    }

    pub(super) fn mark_turn_start(&self, event_tx: &broadcast::Sender<AgentEvent>) -> bool {
        let mut state = self.inner.lock();
        if state.turn_active {
            return false;
        }
        state.turn_active = true;
        let index = state.turn_index;
        drop(state);
        event_tx.send(AgentEvent::TurnStart { index }).ok();
        true
    }

    pub(super) fn finish_turn(&self, event_tx: &broadcast::Sender<AgentEvent>) {
        let mut state = self.inner.lock();
        if !state.turn_active || state.pending_tool_count != 0 {
            state.turn_active = false;
            return;
        }
        let assistant = match state.last_assistant.clone() {
            Some(a) => a,
            None => {
                state.turn_active = false;
                return;
            }
        };
        let turn_index = state.turn_index;
        state.turn_active = false;
        state.last_assistant = None;
        state.batch_tool_results.clear();
        state.turn_index = state.turn_index.saturating_add(1);
        drop(state);

        event_tx
            .send(AgentEvent::TurnEnd {
                index: turn_index,
                message: assistant,
                tool_results: Vec::new(),
            })
            .ok();
    }

    pub(super) fn active_model(&self) -> String {
        self.inner.lock().active_model.clone()
    }

    pub(super) fn set_active_model(&self, model: String) {
        self.inner.lock().active_model = model;
    }

    pub(super) fn cumulative_usage(&self) -> Usage {
        self.inner.lock().cumulative_usage.clone()
    }

    pub(super) fn update_cumulative_usage(&self, f: impl FnOnce(&mut Usage)) {
        let mut state = self.inner.lock();
        f(&mut state.cumulative_usage);
    }
}
