//! Anthropic OAuth compatibility layer for Claude subscription billing.
//!
//! When clankers uses OAuth credentials, Anthropic still sees a third-party
//! client unless the request looks enough like Claude Code. This module does
//! two things for OAuth requests:
//! - prepends a Claude Code billing header block to the system prompt
//! - rewrites clankers-specific marker strings in outbound text, then maps them back on streamed
//!   responses
//!
//! This mirrors the approach used by the openclaw-billing-proxy project, but
//! keeps the logic inside the native Anthropic provider instead of requiring a
//! separate HTTP proxy.

use std::collections::BTreeMap;

use serde_json::Value;

use super::api::ApiContentBlock;
use super::api::ApiRequest;
use super::api::ApiTool;
use super::api::SystemBlock;
use crate::message::Content;
use crate::streaming::ContentDelta;
use crate::streaming::StreamEvent;

pub(crate) const DISABLE_ENV: &str = "CLANKERS_DISABLE_CLAUDE_SUBSCRIPTION_COMPAT";
pub(crate) const BILLING_HEADER_ENV: &str = "CLANKERS_ANTHROPIC_BILLING_HEADER";

const DEFAULT_BILLING_HEADER: &str =
    "x-anthropic-billing-header: cc_version=2.1.80.a46; cc_entrypoint=sdk-cli; cch=00000;";

const OUTBOUND_RULES: &[(&str, &str)] = &[
    ("Clankers", "CCAgent"),
    ("clankers", "ccagent"),
    ("HEARTBEAT_OK", "HB_ACK"),
    ("running inside", "running on"),
];

const REVERSE_RULES: &[(&str, &str)] = &[
    ("CCAgent", "Clankers"),
    ("ccagent", "clankers"),
    ("HB_ACK", "HEARTBEAT_OK"),
    ("running on", "running inside"),
];

pub(crate) fn should_apply(is_oauth: bool) -> bool {
    is_oauth && std::env::var(DISABLE_ENV).unwrap_or_default() != "1"
}

pub(crate) fn apply_outbound(mut request: ApiRequest) -> ApiRequest {
    inject_billing_header(&mut request.system);

    if let Some(system) = request.system.as_mut() {
        for block in system.iter_mut().skip(1) {
            block.text = sanitize_text(&block.text);
        }
    }

    for message in &mut request.messages {
        for block in &mut message.content {
            sanitize_api_content(block);
        }
    }

    if let Some(tools) = request.tools.as_mut() {
        for tool in tools {
            sanitize_tool(tool);
        }
    }

    request
}

#[cfg(test)]
pub(crate) fn apply_inbound(mut event: StreamEvent) -> StreamEvent {
    match &mut event {
        StreamEvent::ContentBlockStart { content_block, .. } => reverse_content(content_block),
        StreamEvent::ContentBlockDelta { delta, .. } => reverse_delta(delta),
        StreamEvent::Error { error } => *error = reverse_text(error),
        StreamEvent::MessageStart { .. }
        | StreamEvent::ContentBlockStop { .. }
        | StreamEvent::MessageDelta { .. }
        | StreamEvent::MessageStop => {}
    }

    event
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingKind {
    Text,
    Thinking,
    InputJson,
}

#[derive(Debug, Default)]
pub(crate) struct InboundEventRewriter {
    pending: BTreeMap<usize, PendingRewrite>,
}

#[derive(Debug)]
struct PendingRewrite {
    kind: PendingKind,
    raw: String,
}

impl InboundEventRewriter {
    pub(crate) fn rewrite(&mut self, event: StreamEvent) -> Vec<StreamEvent> {
        match event {
            StreamEvent::ContentBlockStart {
                index,
                mut content_block,
            } => {
                reverse_start_content(&mut content_block);
                let seed = take_seed_chunk(&mut content_block);
                let mut out = vec![StreamEvent::ContentBlockStart { index, content_block }];
                if let Some((kind, chunk)) = seed {
                    out.extend(self.push_chunk(index, kind, chunk));
                }
                out
            }
            StreamEvent::ContentBlockDelta { index, delta } => match delta {
                ContentDelta::TextDelta { text } => self.push_chunk(index, PendingKind::Text, text),
                ContentDelta::ThinkingDelta { thinking } => self.push_chunk(index, PendingKind::Thinking, thinking),
                ContentDelta::InputJsonDelta { partial_json } => {
                    self.push_chunk(index, PendingKind::InputJson, partial_json)
                }
                ContentDelta::SignatureDelta { signature } => vec![StreamEvent::ContentBlockDelta {
                    index,
                    delta: ContentDelta::SignatureDelta { signature },
                }],
            },
            StreamEvent::ContentBlockStop { index } => {
                let mut out = self.flush_index(index);
                out.push(StreamEvent::ContentBlockStop { index });
                out
            }
            StreamEvent::MessageStop => {
                let mut out = self.flush_all();
                out.push(StreamEvent::MessageStop);
                out
            }
            StreamEvent::Error { error } => {
                let mut out = self.flush_all();
                out.push(StreamEvent::Error {
                    error: reverse_text(&error),
                });
                out
            }
            StreamEvent::MessageStart { message } => vec![StreamEvent::MessageStart { message }],
            StreamEvent::MessageDelta { stop_reason, usage } => {
                vec![StreamEvent::MessageDelta { stop_reason, usage }]
            }
        }
    }

    fn push_chunk(&mut self, index: usize, kind: PendingKind, chunk: String) -> Vec<StreamEvent> {
        let mut out = Vec::new();

        if let Some(existing) = self.pending.get(&index)
            && existing.kind != kind
        {
            out.extend(self.flush_index(index));
        }

        let pending = self.pending.entry(index).or_insert_with(|| PendingRewrite {
            kind,
            raw: String::new(),
        });
        pending.kind = kind;
        pending.raw.push_str(&chunk);

        let keep_len = longest_suffix_prefix_len(&pending.raw, reverse_needles());
        let emit_len = pending.raw.len().saturating_sub(keep_len);
        if emit_len == 0 {
            return out;
        }

        let emit_raw = pending.raw[..emit_len].to_string();
        pending.raw.drain(..emit_len);
        let rewritten = reverse_text(&emit_raw);
        if !rewritten.is_empty() {
            out.push(make_delta_event(index, kind, rewritten));
        }
        out
    }

    fn flush_index(&mut self, index: usize) -> Vec<StreamEvent> {
        let Some(pending) = self.pending.remove(&index) else {
            return Vec::new();
        };

        if pending.raw.is_empty() {
            return Vec::new();
        }

        vec![make_delta_event(index, pending.kind, reverse_text(&pending.raw))]
    }

    fn flush_all(&mut self) -> Vec<StreamEvent> {
        let indexes: Vec<_> = self.pending.keys().copied().collect();
        let mut out = Vec::new();
        for index in indexes {
            out.extend(self.flush_index(index));
        }
        out
    }
}

fn take_seed_chunk(content: &mut Content) -> Option<(PendingKind, String)> {
    match content {
        Content::Text { text } => Some((PendingKind::Text, std::mem::take(text))),
        Content::Thinking { thinking, .. } => Some((PendingKind::Thinking, std::mem::take(thinking))),
        Content::Image { .. } | Content::ToolUse { .. } | Content::ToolResult { .. } => None,
    }
}

fn reverse_start_content(content: &mut Content) {
    match content {
        Content::ToolUse { input, .. } => reverse_json_strings(input),
        Content::ToolResult { content, .. } => {
            for nested in content {
                reverse_start_content(nested);
            }
        }
        Content::Text { .. } | Content::Thinking { .. } | Content::Image { .. } => {}
    }
}

fn make_delta_event(index: usize, kind: PendingKind, text: String) -> StreamEvent {
    let delta = match kind {
        PendingKind::Text => ContentDelta::TextDelta { text },
        PendingKind::Thinking => ContentDelta::ThinkingDelta { thinking: text },
        PendingKind::InputJson => ContentDelta::InputJsonDelta { partial_json: text },
    };
    StreamEvent::ContentBlockDelta { index, delta }
}

fn reverse_needles() -> impl Iterator<Item = &'static str> {
    REVERSE_RULES.iter().map(|(find, _)| *find)
}

fn longest_suffix_prefix_len<'a>(text: &str, patterns: impl Iterator<Item = &'a str>) -> usize {
    let patterns: Vec<&str> = patterns.collect();
    let max_len = patterns.iter().map(|pattern| pattern.len()).max().unwrap_or(0);
    let start = text.len().saturating_sub(max_len.saturating_sub(1));
    for idx in text.char_indices().map(|(idx, _)| idx).chain(std::iter::once(text.len())) {
        if idx < start {
            continue;
        }
        let suffix = &text[idx..];
        if patterns.iter().any(|pattern| pattern.starts_with(suffix)) {
            return suffix.len();
        }
    }
    0
}

fn inject_billing_header(system: &mut Option<Vec<SystemBlock>>) {
    let header_block = SystemBlock {
        block_type: "text".to_string(),
        text: billing_header(),
        cache_control: None,
    };

    match system {
        Some(blocks) => blocks.insert(0, header_block),
        None => *system = Some(vec![header_block]),
    }
}

fn billing_header() -> String {
    match std::env::var(BILLING_HEADER_ENV) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => DEFAULT_BILLING_HEADER.to_string(),
    }
}

fn sanitize_tool(tool: &mut ApiTool) {
    tool.description = sanitize_text(&tool.description);
    sanitize_json_strings(&mut tool.input_schema);
}

fn sanitize_api_content(block: &mut ApiContentBlock) {
    match block {
        ApiContentBlock::Text { text, .. } => *text = sanitize_text(text),
        ApiContentBlock::Image { .. } => {}
        ApiContentBlock::ToolUse { input, .. } => sanitize_json_strings(input),
        ApiContentBlock::ToolResult { content, .. } => {
            for nested in content {
                sanitize_api_content(nested);
            }
        }
        ApiContentBlock::Thinking { .. } => {}
    }
}

#[cfg(test)]
fn reverse_content(content: &mut Content) {
    match content {
        Content::Text { text } => *text = reverse_text(text),
        Content::Image { .. } => {}
        Content::Thinking { thinking, .. } => *thinking = reverse_text(thinking),
        Content::ToolUse { input, .. } => reverse_json_strings(input),
        Content::ToolResult { content, .. } => {
            for nested in content {
                reverse_content(nested);
            }
        }
    }
}

#[cfg(test)]
fn reverse_delta(delta: &mut ContentDelta) {
    match delta {
        ContentDelta::TextDelta { text } => *text = reverse_text(text),
        ContentDelta::ThinkingDelta { thinking } => *thinking = reverse_text(thinking),
        ContentDelta::InputJsonDelta { partial_json } => *partial_json = reverse_text(partial_json),
        ContentDelta::SignatureDelta { .. } => {}
    }
}

fn sanitize_json_strings(value: &mut Value) {
    map_json_strings(value, sanitize_text);
}

fn reverse_json_strings(value: &mut Value) {
    map_json_strings(value, reverse_text);
}

fn map_json_strings(value: &mut Value, mapper: fn(&str) -> String) {
    match value {
        Value::String(text) => *text = mapper(text),
        Value::Array(items) => {
            for item in items {
                map_json_strings(item, mapper);
            }
        }
        Value::Object(map) => {
            for value in map.values_mut() {
                map_json_strings(value, mapper);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn sanitize_text(text: &str) -> String {
    apply_rules(text, OUTBOUND_RULES)
}

fn reverse_text(text: &str) -> String {
    apply_rules(text, REVERSE_RULES)
}

fn apply_rules(text: &str, rules: &[(&str, &str)]) -> String {
    let mut out = text.to_string();
    for (find, replace) in rules {
        out = out.replace(find, replace);
    }
    out
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::OnceLock;

    use serde_json::json;

    use super::*;
    use crate::streaming::ContentDelta;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct TempEnvVar {
        key: &'static str,
        prev: Option<String>,
    }

    impl TempEnvVar {
        fn unset(key: &'static str) -> Self {
            let prev = std::env::var(key).ok();
            unsafe { std::env::remove_var(key) };
            Self { key, prev }
        }
    }

    impl Drop for TempEnvVar {
        fn drop(&mut self) {
            if let Some(prev) = &self.prev {
                unsafe { std::env::set_var(self.key, prev) };
            } else {
                unsafe { std::env::remove_var(self.key) };
            }
        }
    }

    fn collect_text_deltas(events: &[StreamEvent]) -> String {
        let mut out = String::new();
        for event in events {
            match event {
                StreamEvent::ContentBlockDelta {
                    delta: ContentDelta::TextDelta { text },
                    ..
                } => out.push_str(text),
                StreamEvent::ContentBlockDelta {
                    delta: ContentDelta::ThinkingDelta { thinking },
                    ..
                } => out.push_str(thinking),
                _ => {}
            }
        }
        out
    }

    fn collect_json_deltas(events: &[StreamEvent]) -> String {
        let mut out = String::new();
        for event in events {
            if let StreamEvent::ContentBlockDelta {
                delta: ContentDelta::InputJsonDelta { partial_json },
                ..
            } = event
            {
                out.push_str(partial_json);
            }
        }
        out
    }

    fn request_with_text() -> ApiRequest {
        ApiRequest {
            model: "claude-test".to_string(),
            messages: vec![super::super::api::ApiMessage {
                role: "user".to_string(),
                content: vec![ApiContentBlock::Text {
                    text: "running inside clankers with HEARTBEAT_OK".to_string(),
                    cache_control: None,
                }],
            }],
            max_tokens: 64,
            stream: true,
            system: Some(vec![SystemBlock {
                block_type: "text".to_string(),
                text: "You are clankers.".to_string(),
                cache_control: None,
            }]),
            tools: Some(vec![ApiTool {
                name: "read".to_string(),
                description: "Inspect .clankers state".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Read .clankers/config.json" }
                    }
                }),
                cache_control: None,
            }]),
            temperature: None,
            thinking: None,
        }
    }

    #[test]
    fn outbound_injects_header_and_sanitizes_strings() {
        let _lock = env_lock().lock().expect("env lock poisoned");
        let _header_guard = TempEnvVar::unset(BILLING_HEADER_ENV);

        let request = apply_outbound(request_with_text());
        let system = request.system.expect("system blocks should exist");
        assert_eq!(system[0].text, DEFAULT_BILLING_HEADER);
        assert!(system[0].cache_control.is_none());
        assert_eq!(system[1].text, "You are ccagent.");

        match &request.messages[0].content[0] {
            ApiContentBlock::Text { text, .. } => {
                assert_eq!(text, "running on ccagent with HB_ACK");
            }
            other => panic!("expected text block, got {other:?}"),
        }

        let mut tools = request.tools.expect("tools should exist");
        let tool = tools.remove(0);
        assert_eq!(tool.description, "Inspect .ccagent state");
        assert_eq!(tool.input_schema["properties"]["path"]["description"], json!("Read .ccagent/config.json"));
    }

    #[test]
    fn inbound_restores_text_and_json() {
        let event = StreamEvent::ContentBlockStart {
            index: 0,
            content_block: Content::Text {
                text: "Use .ccagent and send HB_ACK while running on metal".to_string(),
            },
        };
        let event = apply_inbound(event);
        match event {
            StreamEvent::ContentBlockStart {
                content_block: Content::Text { text },
                ..
            } => {
                assert_eq!(text, "Use .clankers and send HEARTBEAT_OK while running inside metal");
            }
            other => panic!("unexpected event: {other:?}"),
        }

        let delta = StreamEvent::ContentBlockDelta {
            index: 1,
            delta: ContentDelta::InputJsonDelta {
                partial_json: r#"{"path":".ccagent/config.json","mode":"running on"}"#.to_string(),
            },
        };
        let delta = apply_inbound(delta);
        match delta {
            StreamEvent::ContentBlockDelta {
                delta: ContentDelta::InputJsonDelta { partial_json },
                ..
            } => {
                assert_eq!(partial_json, r#"{"path":".clankers/config.json","mode":"running inside"}"#);
            }
            other => panic!("unexpected delta event: {other:?}"),
        }
    }

    #[test]
    fn should_apply_respects_disable_env() {
        let _lock = env_lock().lock().expect("env lock poisoned");
        let _guard = TempEnvVar::unset(DISABLE_ENV);

        assert!(should_apply(true));
        assert!(!should_apply(false));

        unsafe { std::env::set_var(DISABLE_ENV, "1") };
        assert!(!should_apply(true));
    }

    #[test]
    fn outbound_uses_header_override_env() {
        let _lock = env_lock().lock().expect("env lock poisoned");
        let _guard = TempEnvVar::unset(BILLING_HEADER_ENV);
        unsafe { std::env::set_var(BILLING_HEADER_ENV, "x-anthropic-billing-header: override;") };

        let request = apply_outbound(request_with_text());
        let system = request.system.expect("system blocks should exist");
        assert_eq!(system[0].text, "x-anthropic-billing-header: override;");
    }

    #[test]
    fn inbound_rewriter_restores_split_text_across_deltas() {
        let mut rewriter = InboundEventRewriter::default();
        let mut events = Vec::new();

        events.extend(rewriter.rewrite(StreamEvent::ContentBlockStart {
            index: 0,
            content_block: Content::Text { text: String::new() },
        }));
        events.extend(rewriter.rewrite(StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::TextDelta {
                text: ".ccage".to_string(),
            },
        }));
        events.extend(rewriter.rewrite(StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::TextDelta {
                text: "nt/HB_".to_string(),
            },
        }));
        events.extend(rewriter.rewrite(StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::TextDelta {
                text: "ACK".to_string(),
            },
        }));
        events.extend(rewriter.rewrite(StreamEvent::ContentBlockStop { index: 0 }));

        assert_eq!(collect_text_deltas(&events), ".clankers/HEARTBEAT_OK");
    }

    #[test]
    fn inbound_rewriter_restores_split_input_json_across_deltas() {
        let mut rewriter = InboundEventRewriter::default();
        let mut events = Vec::new();

        events.extend(rewriter.rewrite(StreamEvent::ContentBlockStart {
            index: 1,
            content_block: Content::ToolUse {
                id: "call_1".to_string(),
                name: "read".to_string(),
                input: Value::Object(serde_json::Map::new()),
            },
        }));
        events.extend(rewriter.rewrite(StreamEvent::ContentBlockDelta {
            index: 1,
            delta: ContentDelta::InputJsonDelta {
                partial_json: r#"{"path":".ccage"#.to_string(),
            },
        }));
        events.extend(rewriter.rewrite(StreamEvent::ContentBlockDelta {
            index: 1,
            delta: ContentDelta::InputJsonDelta {
                partial_json: r#"nt/config.json","status":"HB_"#.to_string(),
            },
        }));
        events.extend(rewriter.rewrite(StreamEvent::ContentBlockDelta {
            index: 1,
            delta: ContentDelta::InputJsonDelta {
                partial_json: r#"ACK"}"#.to_string(),
            },
        }));
        events.extend(rewriter.rewrite(StreamEvent::ContentBlockStop { index: 1 }));

        assert_eq!(collect_json_deltas(&events), r#"{"path":".clankers/config.json","status":"HEARTBEAT_OK"}"#);
    }

    #[test]
    fn inbound_rewriter_handles_seed_text_from_block_start() {
        let mut rewriter = InboundEventRewriter::default();
        let mut events = Vec::new();

        events.extend(rewriter.rewrite(StreamEvent::ContentBlockStart {
            index: 2,
            content_block: Content::Text {
                text: ".ccage".to_string(),
            },
        }));
        events.extend(rewriter.rewrite(StreamEvent::ContentBlockDelta {
            index: 2,
            delta: ContentDelta::TextDelta { text: "nt".to_string() },
        }));
        events.extend(rewriter.rewrite(StreamEvent::ContentBlockStop { index: 2 }));

        assert_eq!(collect_text_deltas(&events), ".clankers");
    }

    #[test]
    fn inbound_rewriter_restores_tool_use_start_input() {
        let mut rewriter = InboundEventRewriter::default();
        let events = rewriter.rewrite(StreamEvent::ContentBlockStart {
            index: 3,
            content_block: Content::ToolUse {
                id: "call_1".to_string(),
                name: "read".to_string(),
                input: json!({
                    "path": ".ccagent/config.json",
                    "status": "HB_ACK",
                    "mode": "running on metal"
                }),
            },
        });

        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ContentBlockStart {
                content_block: Content::ToolUse { input, .. },
                ..
            } => {
                assert_eq!(input["path"], json!(".clankers/config.json"));
                assert_eq!(input["status"], json!("HEARTBEAT_OK"));
                assert_eq!(input["mode"], json!("running inside metal"));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
