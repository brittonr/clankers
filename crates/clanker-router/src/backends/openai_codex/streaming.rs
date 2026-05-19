use std::collections::HashMap;

use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;
use tracing::warn;

use super::common;
use crate::error::Error;
use crate::error::Result;
use crate::provider::Usage;
use crate::streaming::ContentBlock;
use crate::streaming::ContentDelta;
use crate::streaming::MessageMetadata;
use crate::streaming::StreamEvent;

enum BlockKind {
    Thinking { buffer: String },
    Text { buffer: String },
    ToolUse { partial_json: String },
}

struct ActiveBlock {
    index: usize,
    kind: BlockKind,
}

pub(crate) struct CodexStreamState {
    model: String,
    sent_start: bool,
    next_index: usize,
    active_blocks: HashMap<String, ActiveBlock>,
    saw_tool_call: bool,
}

impl CodexStreamState {
    pub(crate) fn new(model: String) -> Self {
        Self {
            model,
            sent_start: false,
            next_index: 0,
            active_blocks: HashMap::new(),
            saw_tool_call: false,
        }
    }

    fn ensure_message_start(&mut self, item: &Value, events: &mut Vec<StreamEvent>) {
        if self.sent_start {
            return;
        }
        let id = item.get("id").and_then(|value| value.as_str()).unwrap_or_default();
        events.push(StreamEvent::MessageStart {
            message: MessageMetadata {
                id: id.to_string(),
                model: self.model.clone(),
                role: "assistant".to_string(),
            },
        });
        self.sent_start = true;
    }

    pub(crate) fn handle_event(&mut self, event: &Value) -> Result<Vec<StreamEvent>> {
        let mut events = Vec::new();
        let Some(event_type) = event.get("type").and_then(|value| value.as_str()) else {
            return Ok(events);
        };

        match event_type {
            "response.output_item.added" => {
                let Some(item) = event.get("item") else {
                    return Ok(events);
                };
                self.ensure_message_start(item, &mut events);
                let Some(item_type) = item.get("type").and_then(|value| value.as_str()) else {
                    return Ok(events);
                };
                let item_id = item
                    .get("id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_else(|| item.get("call_id").and_then(|value| value.as_str()).unwrap_or_default())
                    .to_string();
                let index = self.next_index;
                self.next_index += 1;
                match item_type {
                    "reasoning" => {
                        self.active_blocks.insert(item_id, ActiveBlock {
                            index,
                            kind: BlockKind::Thinking { buffer: String::new() },
                        });
                        events.push(StreamEvent::ContentBlockStart {
                            index,
                            content_block: ContentBlock::Thinking {
                                thinking: String::new(),
                                signature: String::new(),
                            },
                        });
                    }
                    "message" => {
                        self.active_blocks.insert(item_id, ActiveBlock {
                            index,
                            kind: BlockKind::Text { buffer: String::new() },
                        });
                        events.push(StreamEvent::ContentBlockStart {
                            index,
                            content_block: ContentBlock::Text { text: String::new() },
                        });
                    }
                    "function_call" => {
                        self.saw_tool_call = true;
                        let call_id = item.get("call_id").and_then(|value| value.as_str()).unwrap_or_default();
                        let name = item.get("name").and_then(|value| value.as_str()).unwrap_or_default();
                        let tool_id = if item_id.is_empty() {
                            call_id.to_string()
                        } else {
                            format!("{call_id}|{item_id}")
                        };
                        let partial_json =
                            item.get("arguments").and_then(|value| value.as_str()).unwrap_or_default().to_string();
                        self.active_blocks.insert(item_id, ActiveBlock {
                            index,
                            kind: BlockKind::ToolUse {
                                partial_json: partial_json.clone(),
                            },
                        });
                        events.push(StreamEvent::ContentBlockStart {
                            index,
                            content_block: ContentBlock::ToolUse {
                                id: tool_id,
                                name: name.to_string(),
                                input: json!({}),
                            },
                        });
                        if !partial_json.is_empty() {
                            events.push(StreamEvent::ContentBlockDelta {
                                index,
                                delta: ContentDelta::InputJsonDelta { partial_json },
                            });
                        }
                    }
                    _ => {}
                }
            }
            "response.reasoning_summary_part.added" => {}
            "response.reasoning_summary_text.delta" => {
                let Some(item_id) = event.get("item_id").and_then(|value| value.as_str()) else {
                    return Ok(events);
                };
                let Some(delta) = event.get("delta").and_then(|value| value.as_str()) else {
                    return Ok(events);
                };
                if let Some(active) = self.active_blocks.get_mut(item_id)
                    && let BlockKind::Thinking { buffer } = &mut active.kind
                {
                    buffer.push_str(delta);
                    events.push(StreamEvent::ContentBlockDelta {
                        index: active.index,
                        delta: ContentDelta::ThinkingDelta {
                            thinking: delta.to_string(),
                        },
                    });
                }
            }
            "response.reasoning_summary_part.done" => {
                let Some(item_id) = event.get("item_id").and_then(|value| value.as_str()) else {
                    return Ok(events);
                };
                if let Some(active) = self.active_blocks.get_mut(item_id)
                    && let BlockKind::Thinking { buffer } = &mut active.kind
                    && !buffer.is_empty()
                {
                    buffer.push_str("\n\n");
                    events.push(StreamEvent::ContentBlockDelta {
                        index: active.index,
                        delta: ContentDelta::ThinkingDelta {
                            thinking: "\n\n".to_string(),
                        },
                    });
                }
            }
            "response.content_part.added" => {}
            "response.output_text.delta" | "response.refusal.delta" => {
                let Some(item_id) = event.get("item_id").and_then(|value| value.as_str()) else {
                    return Ok(events);
                };
                let Some(delta) = event.get("delta").and_then(|value| value.as_str()) else {
                    return Ok(events);
                };
                if let Some(active) = self.active_blocks.get_mut(item_id)
                    && let BlockKind::Text { buffer } = &mut active.kind
                {
                    buffer.push_str(delta);
                    events.push(StreamEvent::ContentBlockDelta {
                        index: active.index,
                        delta: ContentDelta::TextDelta {
                            text: delta.to_string(),
                        },
                    });
                }
            }
            "response.function_call_arguments.delta" => {
                let Some(item_id) = event.get("item_id").and_then(|value| value.as_str()) else {
                    return Ok(events);
                };
                let Some(delta) = event.get("delta").and_then(|value| value.as_str()) else {
                    return Ok(events);
                };
                if let Some(active) = self.active_blocks.get_mut(item_id)
                    && let BlockKind::ToolUse { partial_json } = &mut active.kind
                {
                    partial_json.push_str(delta);
                    events.push(StreamEvent::ContentBlockDelta {
                        index: active.index,
                        delta: ContentDelta::InputJsonDelta {
                            partial_json: delta.to_string(),
                        },
                    });
                }
            }
            "response.function_call_arguments.done" => {
                let Some(item_id) = event.get("item_id").and_then(|value| value.as_str()) else {
                    return Ok(events);
                };
                let Some(arguments) = event.get("arguments").and_then(|value| value.as_str()) else {
                    return Ok(events);
                };
                if let Some(active) = self.active_blocks.get_mut(item_id)
                    && let BlockKind::ToolUse { partial_json } = &mut active.kind
                    && arguments.starts_with(partial_json.as_str())
                {
                    let suffix = &arguments[partial_json.len()..];
                    if !suffix.is_empty() {
                        partial_json.push_str(suffix);
                        events.push(StreamEvent::ContentBlockDelta {
                            index: active.index,
                            delta: ContentDelta::InputJsonDelta {
                                partial_json: suffix.to_string(),
                            },
                        });
                    }
                }
            }
            "response.output_item.done" => {
                let Some(item) = event.get("item") else {
                    return Ok(events);
                };
                let item_id = item
                    .get("id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_else(|| item.get("call_id").and_then(|value| value.as_str()).unwrap_or_default())
                    .to_string();
                let Some(active) = self.active_blocks.remove(&item_id) else {
                    return Ok(events);
                };
                match active.kind {
                    BlockKind::Thinking { mut buffer } => {
                        if buffer.is_empty()
                            && let Some(summary) = item.get("summary").and_then(|value| value.as_array())
                        {
                            buffer = summary
                                .iter()
                                .filter_map(|part| part.get("text").and_then(|value| value.as_str()))
                                .collect::<Vec<_>>()
                                .join("\n\n");
                            if !buffer.is_empty() {
                                events.push(StreamEvent::ContentBlockDelta {
                                    index: active.index,
                                    delta: ContentDelta::ThinkingDelta {
                                        thinking: buffer.clone(),
                                    },
                                });
                            }
                        }
                        events.push(StreamEvent::ContentBlockDelta {
                            index: active.index,
                            delta: ContentDelta::SignatureDelta {
                                signature: serde_json::to_string(item).unwrap_or_else(|_| "{}".to_string()),
                            },
                        });
                        events.push(StreamEvent::ContentBlockStop { index: active.index });
                    }
                    BlockKind::Text { mut buffer } => {
                        if buffer.is_empty()
                            && let Some(content) = item.get("content").and_then(|value| value.as_array())
                        {
                            buffer = content
                                .iter()
                                .filter_map(|part| match part.get("type").and_then(|value| value.as_str()) {
                                    Some("output_text") => part.get("text").and_then(|value| value.as_str()),
                                    Some("refusal") => part.get("refusal").and_then(|value| value.as_str()),
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join("");
                            if !buffer.is_empty() {
                                events.push(StreamEvent::ContentBlockDelta {
                                    index: active.index,
                                    delta: ContentDelta::TextDelta { text: buffer.clone() },
                                });
                            }
                        }
                        events.push(StreamEvent::ContentBlockStop { index: active.index });
                    }
                    BlockKind::ToolUse { partial_json } => {
                        if let Some(arguments) = item.get("arguments").and_then(|value| value.as_str())
                            && arguments.starts_with(partial_json.as_str())
                        {
                            let suffix = &arguments[partial_json.len()..];
                            if !suffix.is_empty() {
                                events.push(StreamEvent::ContentBlockDelta {
                                    index: active.index,
                                    delta: ContentDelta::InputJsonDelta {
                                        partial_json: suffix.to_string(),
                                    },
                                });
                            }
                        }
                        events.push(StreamEvent::ContentBlockStop { index: active.index });
                    }
                }
            }
            "response.completed" | "response.done" => {
                let Some(response) = event.get("response") else {
                    return Ok(events);
                };
                let status = response.get("status").and_then(|value| value.as_str());
                match status {
                    Some("failed" | "cancelled") => {
                        return Err(Error::Provider {
                            message: response
                                .get("error")
                                .and_then(|value| value.get("message"))
                                .and_then(|value| value.as_str())
                                .unwrap_or("Codex response failed")
                                .to_string(),
                            status: Some(500),
                        });
                    }
                    Some("completed" | "incomplete" | "queued" | "in_progress") | None => {}
                    Some(other) => {
                        warn!("unexpected Codex response status '{other}'");
                    }
                }

                let (input_tokens, cache_read_tokens) = response
                    .get("usage")
                    .map(|usage| {
                        let cached = usage
                            .get("input_tokens_details")
                            .and_then(|details| details.get("cached_tokens"))
                            .and_then(|value| value.as_u64())
                            .unwrap_or(0) as usize;
                        let input = usage.get("input_tokens").and_then(|value| value.as_u64()).unwrap_or(0) as usize;
                        (input.saturating_sub(cached), cached)
                    })
                    .unwrap_or((0, 0));
                let output_tokens = response
                    .get("usage")
                    .and_then(|usage| usage.get("output_tokens"))
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0) as usize;
                let stop_reason = match status {
                    Some("completed") if self.saw_tool_call => Some("tool_use".to_string()),
                    Some("completed") => Some("end_turn".to_string()),
                    Some("incomplete") => Some("max_tokens".to_string()),
                    _ => None,
                };
                events.push(StreamEvent::MessageDelta {
                    stop_reason,
                    usage: Usage {
                        input_tokens,
                        output_tokens,
                        cache_read_input_tokens: cache_read_tokens,
                        ..Default::default()
                    },
                });
                events.push(StreamEvent::MessageStop);
            }
            "error" => {
                return Err(Error::Provider {
                    message: event
                        .get("message")
                        .and_then(|value| value.as_str())
                        .unwrap_or("Codex stream error")
                        .to_string(),
                    status: None,
                });
            }
            "response.failed" => {
                return Err(Error::Provider {
                    message: event
                        .get("response")
                        .and_then(|value| value.get("error"))
                        .and_then(|value| value.get("message"))
                        .and_then(|value| value.as_str())
                        .unwrap_or("Codex response failed")
                        .to_string(),
                    status: Some(500),
                });
            }
            _ => {}
        }

        Ok(events)
    }
}

pub(crate) async fn parse_codex_sse(
    response: reqwest::Response,
    model: &str,
    tx: mpsc::Sender<StreamEvent>,
) -> Result<()> {
    let mut reader = common::SseLineReader::new(response);
    let mut state = CodexStreamState::new(model.to_string());

    while let Some(event) = reader.next_event().await? {
        if event.data == "[DONE]" {
            break;
        }
        let value: Value = match serde_json::from_str(&event.data) {
            Ok(value) => value,
            Err(e) => {
                warn!("Failed to parse Codex SSE chunk: {e}: {}", event.data);
                continue;
            }
        };

        let events = state.handle_event(&value)?;
        for stream_event in events {
            if tx.send(stream_event).await.is_err() {
                break;
            }
        }
    }

    Ok(())
}
