//! Provider-neutral stream accumulation for engine host model responses.

use std::collections::BTreeMap;

use clanker_message::{Content, StopReason, Usage};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const EMPTY_TOOL_INPUT_JSON: &str = "{}";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderStreamError {
    pub message: String,
    pub status: Option<u16>,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HostStreamEvent {
    TextStart { index: usize },
    TextDelta { index: usize, text: String },
    ThinkingStart { index: usize, signature: String },
    ThinkingDelta { index: usize, thinking: String },
    ToolUseStart { index: usize, id: String, name: String },
    ToolUseJsonDelta { index: usize, json: String },
    ContentBlockStop { index: usize },
    Usage { usage: Usage },
    MessageStop { model: Option<String>, stop_reason: StopReason },
    ProviderError { error: ProviderStreamError },
}

#[derive(Debug, Clone)]
pub struct StreamFoldResult {
    pub content: Vec<Content>,
    pub usage: Option<Usage>,
    pub model: Option<String>,
    pub stop_reason: Option<StopReason>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StreamAccumulatorError {
    #[error("missing content block start for index {index}")]
    MissingContentBlockStart { index: usize },
    #[error("duplicate content block index {index}")]
    DuplicateContentBlockIndex { index: usize },
    #[error("late content delta for stopped block index {index}")]
    LateContentDelta { index: usize },
    #[error("wrong content delta kind for block index {index}")]
    WrongContentDeltaKind { index: usize },
    #[error("malformed tool JSON for block index {index}: {message}")]
    MalformedToolJson { index: usize, message: String },
    #[error("non-object tool JSON for block index {index}")]
    NonObjectToolJson { index: usize },
    #[error("provider stream error: {message}")]
    ProviderError {
        message: String,
        status: Option<u16>,
        retryable: bool,
    },
}

#[derive(Debug, Clone)]
pub struct StreamAccumulator {
    blocks: BTreeMap<usize, BlockState>,
    usage: Option<Usage>,
    model: Option<String>,
    stop_reason: Option<StopReason>,
}

impl StreamAccumulator {
    #[must_use]
    pub fn new() -> Self {
        Self {
            blocks: BTreeMap::new(),
            usage: None,
            model: None,
            stop_reason: None,
        }
    }

    pub fn push(&mut self, event: HostStreamEvent) -> Result<(), StreamAccumulatorError> {
        match event {
            HostStreamEvent::TextStart { index } => self.insert_block(index, BlockState::Text(TextBlock::default())),
            HostStreamEvent::TextDelta { index, text } => self.push_text_delta(index, text),
            HostStreamEvent::ThinkingStart { index, signature } => {
                self.insert_block(index, BlockState::Thinking(ThinkingBlock { thinking: String::new(), signature, stopped: false }))
            }
            HostStreamEvent::ThinkingDelta { index, thinking } => self.push_thinking_delta(index, thinking),
            HostStreamEvent::ToolUseStart { index, id, name } => self.insert_block(
                index,
                BlockState::ToolUse(ToolUseBlock {
                    id,
                    name,
                    json: String::new(),
                    stopped: false,
                }),
            ),
            HostStreamEvent::ToolUseJsonDelta { index, json } => self.push_tool_json_delta(index, json),
            HostStreamEvent::ContentBlockStop { index } => self.stop_block(index),
            HostStreamEvent::Usage { usage } => {
                self.usage = Some(usage);
                Ok(())
            }
            HostStreamEvent::MessageStop { model, stop_reason } => {
                self.model = model;
                self.stop_reason = Some(stop_reason);
                Ok(())
            }
            HostStreamEvent::ProviderError { error } => Err(StreamAccumulatorError::ProviderError {
                message: error.message,
                status: error.status,
                retryable: error.retryable,
            }),
        }
    }

    pub fn finish(&self) -> Result<StreamFoldResult, StreamAccumulatorError> {
        let mut content = Vec::new();
        for (index, block) in &self.blocks {
            content.push(block_to_content(*index, block)?);
        }
        Ok(StreamFoldResult {
            content,
            usage: self.usage.clone(),
            model: self.model.clone(),
            stop_reason: self.stop_reason.clone(),
        })
    }

    fn insert_block(&mut self, index: usize, block: BlockState) -> Result<(), StreamAccumulatorError> {
        if self.blocks.contains_key(&index) {
            return Err(StreamAccumulatorError::DuplicateContentBlockIndex { index });
        }
        self.blocks.insert(index, block);
        Ok(())
    }

    fn block_mut(&mut self, index: usize) -> Result<&mut BlockState, StreamAccumulatorError> {
        self.blocks
            .get_mut(&index)
            .ok_or(StreamAccumulatorError::MissingContentBlockStart { index })
    }

    fn push_text_delta(&mut self, index: usize, text: String) -> Result<(), StreamAccumulatorError> {
        let block = self.block_mut(index)?;
        ensure_not_stopped(index, block)?;
        let BlockState::Text(text_block) = block else {
            return Err(StreamAccumulatorError::WrongContentDeltaKind { index });
        };
        text_block.text.push_str(&text);
        Ok(())
    }

    fn push_thinking_delta(&mut self, index: usize, thinking: String) -> Result<(), StreamAccumulatorError> {
        let block = self.block_mut(index)?;
        ensure_not_stopped(index, block)?;
        let BlockState::Thinking(thinking_block) = block else {
            return Err(StreamAccumulatorError::WrongContentDeltaKind { index });
        };
        thinking_block.thinking.push_str(&thinking);
        Ok(())
    }

    fn push_tool_json_delta(&mut self, index: usize, json: String) -> Result<(), StreamAccumulatorError> {
        let block = self.block_mut(index)?;
        ensure_not_stopped(index, block)?;
        let BlockState::ToolUse(tool_block) = block else {
            return Err(StreamAccumulatorError::WrongContentDeltaKind { index });
        };
        tool_block.json.push_str(&json);
        Ok(())
    }

    fn stop_block(&mut self, index: usize) -> Result<(), StreamAccumulatorError> {
        let block = self.block_mut(index)?;
        block.stop();
        Ok(())
    }
}

impl Default for StreamAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
enum BlockState {
    Text(TextBlock),
    Thinking(ThinkingBlock),
    ToolUse(ToolUseBlock),
}

impl BlockState {
    fn stopped(&self) -> bool {
        match self {
            Self::Text(block) => block.stopped,
            Self::Thinking(block) => block.stopped,
            Self::ToolUse(block) => block.stopped,
        }
    }

    fn stop(&mut self) {
        match self {
            Self::Text(block) => block.stopped = true,
            Self::Thinking(block) => block.stopped = true,
            Self::ToolUse(block) => block.stopped = true,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct TextBlock {
    text: String,
    stopped: bool,
}

#[derive(Debug, Clone)]
struct ThinkingBlock {
    thinking: String,
    signature: String,
    stopped: bool,
}

#[derive(Debug, Clone)]
struct ToolUseBlock {
    id: String,
    name: String,
    json: String,
    stopped: bool,
}

fn ensure_not_stopped(index: usize, block: &BlockState) -> Result<(), StreamAccumulatorError> {
    if block.stopped() {
        return Err(StreamAccumulatorError::LateContentDelta { index });
    }
    Ok(())
}

fn block_to_content(index: usize, block: &BlockState) -> Result<Content, StreamAccumulatorError> {
    match block {
        BlockState::Text(text) => Ok(Content::Text { text: text.text.clone() }),
        BlockState::Thinking(thinking) => Ok(Content::Thinking {
            thinking: thinking.thinking.clone(),
            signature: thinking.signature.clone(),
        }),
        BlockState::ToolUse(tool) => Ok(Content::ToolUse {
            id: tool.id.clone(),
            name: tool.name.clone(),
            input: parse_tool_input(index, &tool.json)?,
        }),
    }
}

fn parse_tool_input(index: usize, json: &str) -> Result<Value, StreamAccumulatorError> {
    let source = if json.trim().is_empty() { EMPTY_TOOL_INPUT_JSON } else { json };
    let value: Value = serde_json::from_str(source).map_err(|error| StreamAccumulatorError::MalformedToolJson {
        index,
        message: error.to_string(),
    })?;
    if !value.is_object() {
        return Err(StreamAccumulatorError::NonObjectToolJson { index });
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT_INDEX: usize = 0;
    const THINKING_INDEX: usize = 1;
    const TOOL_INDEX: usize = 2;
    const STATUS_TOO_MANY_REQUESTS: u16 = 429;
    const INPUT_TOKENS: usize = 11;
    const OUTPUT_TOKENS: usize = 13;

    #[test]
    fn folds_text_thinking_tool_usage_model_and_stop() {
        let mut acc = StreamAccumulator::new();
        acc.push(HostStreamEvent::TextStart { index: TEXT_INDEX }).unwrap();
        acc.push(HostStreamEvent::TextDelta { index: TEXT_INDEX, text: "hel".to_string() }).unwrap();
        acc.push(HostStreamEvent::TextDelta { index: TEXT_INDEX, text: "lo".to_string() }).unwrap();
        acc.push(HostStreamEvent::ContentBlockStop { index: TEXT_INDEX }).unwrap();
        acc.push(HostStreamEvent::ThinkingStart { index: THINKING_INDEX, signature: "sig".to_string() }).unwrap();
        acc.push(HostStreamEvent::ThinkingDelta { index: THINKING_INDEX, thinking: "think".to_string() }).unwrap();
        acc.push(HostStreamEvent::ToolUseStart { index: TOOL_INDEX, id: "call".to_string(), name: "tool".to_string() }).unwrap();
        acc.push(HostStreamEvent::ToolUseJsonDelta { index: TOOL_INDEX, json: "{\"x\":".to_string() }).unwrap();
        acc.push(HostStreamEvent::ToolUseJsonDelta { index: TOOL_INDEX, json: "1}".to_string() }).unwrap();
        acc.push(HostStreamEvent::Usage { usage: usage() }).unwrap();
        acc.push(HostStreamEvent::MessageStop { model: Some("model".to_string()), stop_reason: StopReason::ToolUse }).unwrap();

        let folded = acc.finish().unwrap();
        assert_eq!(folded.content.len(), 3);
        assert_eq!(folded.model.as_deref(), Some("model"));
        assert_eq!(folded.stop_reason, Some(StopReason::ToolUse));
        assert_eq!(folded.usage.unwrap().input_tokens, INPUT_TOKENS);
        assert_text(&folded.content[0], "hello");
        assert_tool(&folded.content[2], "call", "tool");
    }

    #[test]
    fn rejects_delta_before_start() {
        let mut acc = StreamAccumulator::new();
        let err = acc.push(HostStreamEvent::TextDelta { index: TEXT_INDEX, text: "late".to_string() }).unwrap_err();
        assert_eq!(err, StreamAccumulatorError::MissingContentBlockStart { index: TEXT_INDEX });
    }

    #[test]
    fn rejects_duplicate_index() {
        let mut acc = StreamAccumulator::new();
        acc.push(HostStreamEvent::TextStart { index: TEXT_INDEX }).unwrap();
        let err = acc.push(HostStreamEvent::TextStart { index: TEXT_INDEX }).unwrap_err();
        assert_eq!(err, StreamAccumulatorError::DuplicateContentBlockIndex { index: TEXT_INDEX });
    }

    #[test]
    fn rejects_late_delta_after_stop() {
        let mut acc = StreamAccumulator::new();
        acc.push(HostStreamEvent::TextStart { index: TEXT_INDEX }).unwrap();
        acc.push(HostStreamEvent::ContentBlockStop { index: TEXT_INDEX }).unwrap();
        let err = acc.push(HostStreamEvent::TextDelta { index: TEXT_INDEX, text: "late".to_string() }).unwrap_err();
        assert_eq!(err, StreamAccumulatorError::LateContentDelta { index: TEXT_INDEX });
    }

    #[test]
    fn rejects_malformed_tool_json() {
        let mut acc = StreamAccumulator::new();
        acc.push(HostStreamEvent::ToolUseStart { index: TOOL_INDEX, id: "call".to_string(), name: "tool".to_string() }).unwrap();
        acc.push(HostStreamEvent::ToolUseJsonDelta { index: TOOL_INDEX, json: "{".to_string() }).unwrap();
        let err = acc.finish().unwrap_err();
        assert!(matches!(err, StreamAccumulatorError::MalformedToolJson { index: TOOL_INDEX, .. }));
    }

    #[test]
    fn rejects_non_object_tool_json() {
        let mut acc = StreamAccumulator::new();
        acc.push(HostStreamEvent::ToolUseStart { index: TOOL_INDEX, id: "call".to_string(), name: "tool".to_string() }).unwrap();
        acc.push(HostStreamEvent::ToolUseJsonDelta { index: TOOL_INDEX, json: "[]".to_string() }).unwrap();
        let err = acc.finish().unwrap_err();
        assert_eq!(err, StreamAccumulatorError::NonObjectToolJson { index: TOOL_INDEX });
    }

    #[test]
    fn preserves_provider_error_status_and_retryability() {
        let mut acc = StreamAccumulator::new();
        let err = acc
            .push(HostStreamEvent::ProviderError {
                error: ProviderStreamError {
                    message: "rate limited".to_string(),
                    status: Some(STATUS_TOO_MANY_REQUESTS),
                    retryable: true,
                },
            })
            .unwrap_err();
        assert_eq!(
            err,
            StreamAccumulatorError::ProviderError {
                message: "rate limited".to_string(),
                status: Some(STATUS_TOO_MANY_REQUESTS),
                retryable: true,
            }
        );
    }

    #[test]
    fn usage_only_and_empty_stop_normalize() {
        let mut acc = StreamAccumulator::new();
        acc.push(HostStreamEvent::Usage { usage: usage() }).unwrap();
        acc.push(HostStreamEvent::MessageStop { model: None, stop_reason: StopReason::Stop }).unwrap();
        let folded = acc.finish().unwrap();
        assert!(folded.content.is_empty());
        assert_eq!(folded.usage.unwrap().output_tokens, OUTPUT_TOKENS);
        assert_eq!(folded.stop_reason, Some(StopReason::Stop));
    }

    fn usage() -> Usage {
        Usage {
            input_tokens: INPUT_TOKENS,
            output_tokens: OUTPUT_TOKENS,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        }
    }

    fn assert_text(content: &Content, expected: &str) {
        let Content::Text { text } = content else {
            panic!("expected text block");
        };
        assert_eq!(text, expected);
    }

    fn assert_tool(content: &Content, expected_id: &str, expected_name: &str) {
        let Content::ToolUse { id, name, input } = content else {
            panic!("expected tool block");
        };
        assert_eq!(id, expected_id);
        assert_eq!(name, expected_name);
        assert_eq!(input.get("x").and_then(Value::as_i64), Some(1));
    }
}
