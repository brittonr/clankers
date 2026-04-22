//! Conversation blocks — Warp-style grouped prompt/response units.

use chrono::DateTime;
use chrono::Utc;
use serde::Serialize;
use serde_json::Value;

use crate::display::DisplayImage;
use crate::display::DisplayMessage;
use crate::display::MessageRole;

/// Current canonical envelope version for conversation-block hashing.
pub const CANONICAL_BLOCK_ENVELOPE_VERSION_V1: u8 = 1;

#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        ambient_clock,
        reason = "synthetic conversation blocks still originate at the shell boundary"
    )
)]
fn synthetic_block_timestamp() -> DateTime<Utc> {
    Utc::now()
}

/// A single conversation block: one user turn + the full agent response.
#[derive(Debug, Clone)]
pub struct ConversationBlock {
    /// Unique block ID (monotonic counter).
    pub id: usize,
    /// Canonical timestamp for the block's opening user message.
    pub started_at: DateTime<Utc>,
    /// Finalized canonical BLAKE3 hash once the block is complete.
    pub finalized_hash: Option<String>,
    /// The user's input prompt.
    pub prompt: String,
    /// All response messages (thinking, assistant text, tool calls/results).
    pub responses: Vec<DisplayMessage>,
    /// Whether the block is collapsed (shows only a summary line).
    pub collapsed: bool,
    /// Whether this block is still streaming (not yet complete).
    pub streaming: bool,
    /// Optional error that terminated this block.
    pub error: Option<String>,
    /// Token usage for this block.
    pub tokens: usize,

    // ── Branching ────────────────────────────────────
    /// ID of the parent block (the block before this one in the conversation).
    /// `None` means this is a root block (first in a conversation).
    pub parent_block_id: Option<usize>,
    /// The number of agent messages at the point just before this block started.
    /// Used to truncate the agent's history when branching.
    pub agent_msg_checkpoint: usize,
}

impl ConversationBlock {
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(
            usize_in_public_api,
            reason = "conversation block IDs are tree indexes shared with existing TUI code"
        )
    )]
    pub fn new(id: usize, prompt: String, started_at: DateTime<Utc>) -> Self {
        Self {
            id,
            started_at,
            finalized_hash: None,
            prompt,
            responses: Vec::new(),
            collapsed: false,
            streaming: true,
            error: None,
            tokens: 0,
            parent_block_id: None,
            agent_msg_checkpoint: 0,
        }
    }

    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(
            ambient_clock,
            reason = "synthetic preview/test blocks do not have persisted message timestamps"
        )
    )]
    pub fn new_synthetic(id: usize, prompt: String) -> Self {
        Self::new(id, prompt, synthetic_block_timestamp())
    }

    pub fn finalize_metadata(&mut self) {
        self.finalized_hash = Some(finalized_block_hash(self.started_at, &self.prompt, &self.responses));
    }

    /// One-line summary for collapsed view.
    pub fn summary(&self) -> String {
        let status = if self.streaming {
            "…"
        } else if self.error.is_some() {
            "✗"
        } else {
            "✓"
        };
        let tool_count = self.responses.iter().filter(|m| m.role == MessageRole::ToolCall).count();
        let text_preview: String = self
            .responses
            .iter()
            .filter(|m| m.role == MessageRole::Assistant)
            .find_map(|m| m.content.lines().next())
            .unwrap_or("...")
            .chars()
            .take(60)
            .collect();

        if tool_count > 0 {
            format!("{} {} ({} tools) — {}", status, self.prompt_preview(), tool_count, text_preview)
        } else {
            format!("{} {} — {}", status, self.prompt_preview(), text_preview)
        }
    }

    fn prompt_preview(&self) -> String {
        self.prompt.lines().next().unwrap_or("").chars().take(40).collect()
    }

    /// Toggle collapsed state.
    pub fn toggle_collapse(&mut self) {
        self.collapsed = !self.collapsed;
    }
}

/// Top-level entry in the conversation: either a block or a standalone system message.
#[derive(Debug, Clone)]
pub enum BlockEntry {
    /// A full prompt→response block.
    Conversation(ConversationBlock),
    /// A standalone system message (not part of a prompt).
    System(DisplayMessage),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct CanonicalBlockEnvelopeV1 {
    v: u8,
    started_at: String,
    items: Vec<CanonicalBlockItemV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind")]
enum CanonicalBlockItemV1 {
    #[serde(rename = "user")]
    User { text: String },
    #[serde(rename = "assistant_text")]
    AssistantText { text: String },
    #[serde(rename = "thinking")]
    Thinking { text: String },
    #[serde(rename = "tool_call")]
    ToolCall { name: String, input: Value },
    #[serde(rename = "tool_result")]
    ToolResult {
        text: String,
        is_error: bool,
        images: Vec<CanonicalImageV1>,
    },
    #[serde(rename = "system")]
    System { text: String, is_error: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct CanonicalImageV1 {
    media_type: String,
    data: String,
}

/// Canonical v1 envelope bytes pinned for review.
///
/// Field order is exactly `v`, `started_at`, `items`, and every item serializes
/// `kind` first followed by the declared stable fields for that kind.
pub fn canonical_block_envelope_v1_bytes(
    started_at: DateTime<Utc>,
    prompt: &str,
    responses: &[DisplayMessage],
) -> Vec<u8> {
    let envelope = CanonicalBlockEnvelopeV1 {
        v: CANONICAL_BLOCK_ENVELOPE_VERSION_V1,
        started_at: started_at.to_rfc3339(),
        items: canonical_block_items(prompt, responses),
    };
    match serde_json::to_vec(&envelope) {
        Ok(bytes) => bytes,
        Err(error) => panic!("canonical conversation block envelope must serialize: {error}"),
    }
}

pub fn finalized_block_hash(started_at: DateTime<Utc>, prompt: &str, responses: &[DisplayMessage]) -> String {
    let canonical_bytes = canonical_block_envelope_v1_bytes(started_at, prompt, responses);
    blake3::hash(&canonical_bytes).to_hex().to_string()
}

fn canonical_block_items(prompt: &str, responses: &[DisplayMessage]) -> Vec<CanonicalBlockItemV1> {
    let response_item_count = responses.len();
    let user_item_count = 1usize;
    let total_item_count = user_item_count + response_item_count;
    let mut items = Vec::with_capacity(total_item_count);
    items.push(CanonicalBlockItemV1::User {
        text: prompt.to_string(),
    });
    for response in responses {
        items.push(response_to_canonical_item(response));
    }
    items
}

fn response_to_canonical_item(response: &DisplayMessage) -> CanonicalBlockItemV1 {
    match response.role {
        MessageRole::Assistant => CanonicalBlockItemV1::AssistantText {
            text: response.content.clone(),
        },
        MessageRole::Thinking => CanonicalBlockItemV1::Thinking {
            text: response.content.clone(),
        },
        MessageRole::ToolCall => CanonicalBlockItemV1::ToolCall {
            name: response.tool_name.clone().unwrap_or_else(|| response.content.clone()),
            input: response.tool_input.clone().unwrap_or(Value::Null),
        },
        MessageRole::ToolResult => CanonicalBlockItemV1::ToolResult {
            text: response.content.clone(),
            is_error: response.is_error,
            images: canonical_images(&response.images),
        },
        MessageRole::System => CanonicalBlockItemV1::System {
            text: response.content.clone(),
            is_error: response.is_error,
        },
        MessageRole::User => CanonicalBlockItemV1::User {
            text: response.content.clone(),
        },
    }
}

fn canonical_images(images: &[DisplayImage]) -> Vec<CanonicalImageV1> {
    images
        .iter()
        .map(|image| CanonicalImageV1 {
            media_type: image.media_type.clone(),
            data: image.data.clone(),
        })
        .collect()
}

/// Implement TreeNode trait for ConversationBlock to enable rat-branches tree algorithms
impl rat_branches::TreeNode for ConversationBlock {
    fn id(&self) -> usize {
        self.id
    }

    fn parent_id(&self) -> Option<usize> {
        self.parent_block_id
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use chrono::Utc;
    use serde_json::json;

    use super::*;

    const FIXED_STARTED_AT: &str = "2026-04-22T12:34:56Z";
    const CHANGED_STARTED_AT: &str = "2026-04-22T12:35:56Z";
    const FIXTURE_PROMPT: &str = "hello block";
    const ORIGINAL_TOKEN_COUNT: usize = 123;
    const CHANGED_TOKEN_COUNT: usize = 456;
    const ORIGINAL_PARENT_BLOCK_ID: usize = 77;
    const CHANGED_PARENT_BLOCK_ID: usize = 22;
    const ORIGINAL_AGENT_MESSAGE_CHECKPOINT: usize = 88;
    const CHANGED_AGENT_MESSAGE_CHECKPOINT: usize = 11;
    const EXPECTED_CANONICAL_ENVELOPE_JSON: &str = r#"{"v":1,"started_at":"2026-04-22T12:34:56+00:00","items":[{"kind":"user","text":"hello block"},{"kind":"assistant_text","text":"assistant reply"},{"kind":"thinking","text":"pondering"},{"kind":"tool_call","name":"bash","input":{"command":"ls","depth":1}},{"kind":"tool_result","text":"tool output","is_error":false,"images":[{"media_type":"image/png","data":"ZmFrZS1wbmc="}]},{"kind":"system","text":"system note","is_error":true}]}"#;

    fn fixed_started_at() -> DateTime<Utc> {
        match DateTime::parse_from_rfc3339(FIXED_STARTED_AT) {
            Ok(timestamp) => timestamp.with_timezone(&Utc),
            Err(error) => panic!("test timestamp must parse: {error}"),
        }
    }

    fn base_responses() -> Vec<DisplayMessage> {
        vec![
            DisplayMessage {
                role: MessageRole::Assistant,
                content: "assistant reply".to_string(),
                tool_name: None,
                tool_input: None,
                is_error: false,
                images: Vec::new(),
            },
            DisplayMessage {
                role: MessageRole::Thinking,
                content: "pondering".to_string(),
                tool_name: None,
                tool_input: None,
                is_error: false,
                images: Vec::new(),
            },
            DisplayMessage {
                role: MessageRole::ToolCall,
                content: "bash".to_string(),
                tool_name: Some("bash".to_string()),
                tool_input: Some(json!({"command": "ls", "depth": 1})),
                is_error: false,
                images: Vec::new(),
            },
            DisplayMessage {
                role: MessageRole::ToolResult,
                content: "tool output".to_string(),
                tool_name: None,
                tool_input: None,
                is_error: false,
                images: vec![DisplayImage {
                    data: "ZmFrZS1wbmc=".to_string(),
                    media_type: "image/png".to_string(),
                }],
            },
            DisplayMessage {
                role: MessageRole::System,
                content: "system note".to_string(),
                tool_name: None,
                tool_input: None,
                is_error: true,
                images: Vec::new(),
            },
        ]
    }

    #[test]
    fn canonical_block_envelope_v1_matches_fixture() {
        let actual_bytes = canonical_block_envelope_v1_bytes(fixed_started_at(), FIXTURE_PROMPT, &base_responses());
        let actual = match String::from_utf8(actual_bytes) {
            Ok(text) => text,
            Err(error) => panic!("canonical block fixture must stay UTF-8: {error}"),
        };

        assert_eq!(actual, EXPECTED_CANONICAL_ENVELOPE_JSON);
    }

    #[test]
    fn identical_canonical_content_yields_same_hash() {
        let first_hash = finalized_block_hash(fixed_started_at(), FIXTURE_PROMPT, &base_responses());
        let second_hash = finalized_block_hash(fixed_started_at(), FIXTURE_PROMPT, &base_responses());

        assert_eq!(first_hash, second_hash);
    }

    #[test]
    fn transient_ui_state_does_not_affect_finalized_hash() {
        let mut original = ConversationBlock::new(1, FIXTURE_PROMPT.to_string(), fixed_started_at());
        original.responses = base_responses();
        original.collapsed = false;
        original.streaming = false;
        original.error = Some("display-only".to_string());
        original.tokens = ORIGINAL_TOKEN_COUNT;
        original.parent_block_id = Some(ORIGINAL_PARENT_BLOCK_ID);
        original.agent_msg_checkpoint = ORIGINAL_AGENT_MESSAGE_CHECKPOINT;

        let mut changed = original.clone();
        changed.id = 999;
        changed.collapsed = true;
        changed.streaming = true;
        changed.error = None;
        changed.tokens = CHANGED_TOKEN_COUNT;
        changed.parent_block_id = Some(CHANGED_PARENT_BLOCK_ID);
        changed.agent_msg_checkpoint = CHANGED_AGENT_MESSAGE_CHECKPOINT;

        let original_hash = finalized_block_hash(original.started_at, &original.prompt, &original.responses);
        let changed_hash = finalized_block_hash(changed.started_at, &changed.prompt, &changed.responses);

        assert_eq!(original_hash, changed_hash);
    }

    #[test]
    fn finalized_hash_changes_when_prompt_changes() {
        let original_hash = finalized_block_hash(fixed_started_at(), FIXTURE_PROMPT, &base_responses());
        let changed_hash = finalized_block_hash(fixed_started_at(), "different prompt", &base_responses());

        assert_ne!(original_hash, changed_hash);
    }

    #[test]
    fn finalized_hash_changes_when_assistant_or_tool_content_changes() {
        let original_hash = finalized_block_hash(fixed_started_at(), FIXTURE_PROMPT, &base_responses());
        let mut changed_responses = base_responses();
        changed_responses[0].content = "changed assistant reply".to_string();
        changed_responses[3].content = "changed tool output".to_string();
        let changed_hash = finalized_block_hash(fixed_started_at(), FIXTURE_PROMPT, &changed_responses);

        assert_ne!(original_hash, changed_hash);
    }

    #[test]
    fn finalized_hash_changes_when_started_at_changes() {
        let original_hash = finalized_block_hash(fixed_started_at(), FIXTURE_PROMPT, &base_responses());
        let changed_started_at = match DateTime::parse_from_rfc3339(CHANGED_STARTED_AT) {
            Ok(timestamp) => timestamp.with_timezone(&Utc),
            Err(error) => panic!("changed test timestamp must parse: {error}"),
        };
        let changed_hash = finalized_block_hash(changed_started_at, FIXTURE_PROMPT, &base_responses());

        assert_ne!(original_hash, changed_hash);
    }
}
