use clankers_provider::message::AgentMessage;
use clankers_provider::message::Content;
use clankers_provider::message::ToolResultMessage;
use serde_json::Value;

const SUMMARY_MARKER_KEY: &str = "_compaction_summary";
const ORIGINAL_DETAILS_KEY: &str = "_compaction_original_details";
const DEFAULT_READ_OFFSET: u64 = 1;
const DEFAULT_EDIT_COUNT: usize = 1;
const ZERO_MATCHES: usize = 0;
const MAX_ARG_PREVIEW_CHARS: usize = 80;
const MAX_TASK_PREVIEW_CHARS: usize = 60;
const UNKNOWN_ARG_PREVIEW: &str = "?";

#[derive(Debug, Clone, Default)]
struct ContentStats {
    joined_text: String,
    char_count: usize,
    line_count: usize,
    has_image: bool,
}

pub fn summarize_tool_result(tool_name: &str, args: &Value, content: &[Content]) -> String {
    let stats = content_stats(content);
    let summary = match tool_name {
        "bash" => summarize_bash(args, &stats),
        "read" => summarize_read(args, &stats),
        "write" => summarize_write(args),
        "grep" | "rg" => summarize_grep(tool_name, args, &stats),
        "edit" => summarize_edit(args),
        "subagent" | "delegate_task" => summarize_subagent(tool_name, args, &stats),
        _ => summarize_generic(tool_name, args, &stats),
    };

    assert!(!summary.is_empty(), "tool summary must not be empty");
    assert!(!summary.contains('\n'), "tool summary must stay single-line");
    summary
}

pub fn prune_tool_results(messages: &[AgentMessage], tail_start_idx: usize) -> Vec<AgentMessage> {
    assert!(tail_start_idx <= messages.len(), "tail_start_idx out of bounds");

    let mut result = Vec::with_capacity(messages.len());
    for (index, message) in messages.iter().enumerate() {
        match message {
            AgentMessage::ToolResult(tool_result) if index < tail_start_idx => {
                if let Some(summary) = existing_summary(tool_result) {
                    result.push(AgentMessage::ToolResult(clone_with_summary(tool_result, summary.to_string())));
                    continue;
                }

                let args = find_tool_args_before(messages, index, &tool_result.call_id);
                let summary = summarize_tool_result(&tool_result.tool_name, &args, &tool_result.content);
                result.push(AgentMessage::ToolResult(clone_with_summary(tool_result, summary)));
            }
            _ => result.push(message.clone()),
        }
    }

    result
}

pub(crate) fn count_prunable_tool_results(messages: &[AgentMessage], tail_start_idx: usize) -> usize {
    assert!(tail_start_idx <= messages.len(), "tail_start_idx out of bounds");

    messages
        .iter()
        .take(tail_start_idx)
        .filter(|message| matches!(message, AgentMessage::ToolResult(tool_result) if existing_summary(tool_result).is_none()))
        .count()
}

fn summarize_bash(args: &Value, stats: &ContentStats) -> String {
    let command = extract_string_arg(args, "command").unwrap_or_else(|| UNKNOWN_ARG_PREVIEW.to_string());
    let command_preview = sanitize_preview(&command, MAX_ARG_PREVIEW_CHARS);
    let (exit_code, output_text) = parse_bash_output(&stats.joined_text);
    let line_count = count_lines(output_text);
    let exit_label = match exit_code {
        Some(code) => format!("exit {code}"),
        None => "exit ?".to_string(),
    };
    format!("[bash] {command_preview} ({exit_label}, {line_count} lines)")
}

fn summarize_read(args: &Value, stats: &ContentStats) -> String {
    let path = extract_string_arg(args, "path").unwrap_or_else(|| UNKNOWN_ARG_PREVIEW.to_string());
    let path_preview = sanitize_preview(&path, MAX_ARG_PREVIEW_CHARS);
    let offset = args.get("offset").and_then(Value::as_u64).unwrap_or(DEFAULT_READ_OFFSET);
    if stats.has_image && stats.char_count == 0 {
        return format!("[read] {path_preview} @{offset} (image result)");
    }
    format!("[read] {path_preview} @{offset} ({} chars)", stats.char_count)
}

fn summarize_write(args: &Value) -> String {
    let path = extract_string_arg(args, "path").unwrap_or_else(|| UNKNOWN_ARG_PREVIEW.to_string());
    let path_preview = sanitize_preview(&path, MAX_ARG_PREVIEW_CHARS);
    let content = extract_string_arg(args, "content").unwrap_or_default();
    let line_count = count_lines(&content);
    format!("[write] {path_preview} ({line_count} lines)")
}

fn summarize_grep(tool_name: &str, args: &Value, stats: &ContentStats) -> String {
    let pattern = extract_string_arg(args, "pattern").unwrap_or_else(|| UNKNOWN_ARG_PREVIEW.to_string());
    let pattern_preview = sanitize_preview(&pattern, MAX_ARG_PREVIEW_CHARS);
    let match_count = if stats.joined_text.trim() == "No matches found" {
        ZERO_MATCHES
    } else {
        stats.line_count
    };
    format!("[{tool_name}] pattern={pattern_preview} ({match_count} matches)")
}

fn summarize_edit(args: &Value) -> String {
    let path = extract_string_arg(args, "path").unwrap_or_else(|| UNKNOWN_ARG_PREVIEW.to_string());
    let path_preview = sanitize_preview(&path, MAX_ARG_PREVIEW_CHARS);
    let edit_count = args
        .get("edits")
        .and_then(Value::as_array)
        .map(Vec::len)
        .or_else(|| {
            if args.get("old_text").is_some() && args.get("new_text").is_some() {
                Some(DEFAULT_EDIT_COUNT)
            } else {
                None
            }
        })
        .unwrap_or_default();
    let label = pluralized(edit_count, "edit", "edits");
    format!("[edit] {path_preview} ({label})")
}

fn summarize_subagent(tool_name: &str, args: &Value, stats: &ContentStats) -> String {
    let goal = if let Some(task) = extract_string_arg(args, "task") {
        format!("task={}", sanitize_preview(&task, MAX_TASK_PREVIEW_CHARS))
    } else if let Some(tasks) = args.get("tasks").and_then(Value::as_array) {
        format!("tasks={}", tasks.len())
    } else if let Some(chain) = args.get("chain").and_then(Value::as_array) {
        format!("chain={}", chain.len())
    } else {
        "task=?".to_string()
    };

    if stats.has_image && stats.char_count == 0 {
        return format!("[{tool_name}] {goal} (image result)");
    }

    format!("[{tool_name}] {goal} ({} chars result)", stats.char_count)
}

fn summarize_generic(tool_name: &str, args: &Value, stats: &ContentStats) -> String {
    let arg_preview = first_arg_preview(args).map_or_else(String::new, |preview| format!(" {preview}"));
    if stats.has_image && stats.char_count == 0 {
        return format!("[{tool_name}]{arg_preview} (image result)");
    }
    format!("[{tool_name}]{arg_preview} ({} chars result)", stats.char_count)
}

fn content_stats(content: &[Content]) -> ContentStats {
    let mut text_parts = Vec::new();
    let mut char_count = 0usize;
    let mut line_count = 0usize;
    let mut has_image = false;

    for block in content {
        match block {
            Content::Text { text } => push_text_stats(text, &mut text_parts, &mut char_count, &mut line_count),
            Content::Thinking { thinking, .. } => {
                push_text_stats(thinking, &mut text_parts, &mut char_count, &mut line_count)
            }
            Content::Image { .. } => {
                has_image = true;
            }
            Content::ToolUse { .. } | Content::ToolResult { .. } => {
                let rendered = serde_json::to_string(block).unwrap_or_default();
                push_text_stats(&rendered, &mut text_parts, &mut char_count, &mut line_count);
            }
        }
    }

    ContentStats {
        joined_text: text_parts.join("\n"),
        char_count,
        line_count,
        has_image,
    }
}

fn push_text_stats(text: &str, text_parts: &mut Vec<String>, char_count: &mut usize, line_count: &mut usize) {
    text_parts.push(text.to_string());
    *char_count += text.chars().count();
    *line_count += count_lines(text);
}

fn count_lines(text: &str) -> usize {
    if text.is_empty() { 0 } else { text.lines().count() }
}

fn parse_bash_output(text: &str) -> (Option<i32>, &str) {
    const EXIT_CODE_PREFIX: &str = "Exit code: ";
    const EXIT_CODE_SEPARATOR: &str = "\n\n";

    if let Some(rest) = text.strip_prefix(EXIT_CODE_PREFIX) {
        if let Some((code_text, output_text)) = rest.split_once(EXIT_CODE_SEPARATOR) {
            if let Ok(code) = code_text.trim().parse::<i32>() {
                return (Some(code), output_text);
            }
        }
    }

    (Some(0), text)
}

fn find_tool_args_before(messages: &[AgentMessage], message_idx: usize, call_id: &str) -> Value {
    for message in messages[..message_idx].iter().rev() {
        if let AgentMessage::Assistant(assistant) = message {
            for block in assistant.content.iter().rev() {
                if let Content::ToolUse { id, input, .. } = block
                    && id == call_id
                {
                    return input.clone();
                }
            }
        }
    }

    Value::Null
}

fn clone_with_summary(tool_result: &ToolResultMessage, summary: String) -> ToolResultMessage {
    ToolResultMessage {
        id: tool_result.id.clone(),
        call_id: tool_result.call_id.clone(),
        tool_name: tool_result.tool_name.clone(),
        content: vec![Content::Text { text: summary.clone() }],
        is_error: tool_result.is_error,
        details: Some(mark_summary(tool_result.details.as_ref(), summary)),
        timestamp: tool_result.timestamp,
    }
}

fn mark_summary(details: Option<&Value>, summary: String) -> Value {
    let mut map = details.and_then(Value::as_object).cloned().unwrap_or_default();
    map.insert(SUMMARY_MARKER_KEY.to_string(), Value::String(summary));

    if let Some(existing) = details
        && !existing.is_object()
    {
        map.insert(ORIGINAL_DETAILS_KEY.to_string(), existing.clone());
    }

    Value::Object(map)
}

fn existing_summary(tool_result: &ToolResultMessage) -> Option<&str> {
    tool_result.details.as_ref()?.get(SUMMARY_MARKER_KEY)?.as_str()
}

fn extract_string_arg(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(ToString::to_string)
}

fn first_arg_preview(args: &Value) -> Option<String> {
    let object = args.as_object()?;
    if object.is_empty() {
        return None;
    }

    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let first_key = keys.first()?;
    let value = object.get(*first_key)?;
    Some(format!("{}={}", first_key, render_value_preview(value)))
}

fn render_value_preview(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => sanitize_preview(text, MAX_ARG_PREVIEW_CHARS),
        Value::Array(values) => format!("{} items", values.len()),
        Value::Object(object) => format!("{} keys", object.len()),
    }
}

fn sanitize_preview(text: &str, max_chars: usize) -> String {
    let single_line = text.replace('\n', " ");
    let preview: String = single_line.chars().take(max_chars).collect();
    if single_line.chars().count() > max_chars {
        format!("{preview}...")
    } else {
        preview
    }
}

fn pluralized(count: usize, singular: &str, plural: &str) -> String {
    if count == DEFAULT_EDIT_COUNT {
        format!("{count} {singular}")
    } else {
        format!("{count} {plural}")
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use clankers_provider::Usage;
    use clankers_provider::message::AssistantMessage;
    use clankers_provider::message::ImageSource;
    use clankers_provider::message::MessageId;
    use clankers_provider::message::StopReason;
    use serde_json::json;

    use super::*;

    fn text_content(text: &str) -> Vec<Content> {
        vec![Content::Text { text: text.to_string() }]
    }

    fn assistant_tool_use(call_id: &str, tool_name: &str, input: Value) -> AgentMessage {
        AgentMessage::Assistant(AssistantMessage {
            id: MessageId::generate(),
            content: vec![Content::ToolUse {
                id: call_id.to_string(),
                name: tool_name.to_string(),
                input,
            }],
            model: "test-model".to_string(),
            usage: Usage::default(),
            stop_reason: StopReason::ToolUse,
            timestamp: Utc::now(),
        })
    }

    fn tool_result(call_id: &str, tool_name: &str, content: Vec<Content>) -> AgentMessage {
        AgentMessage::ToolResult(ToolResultMessage {
            id: MessageId::generate(),
            call_id: call_id.to_string(),
            tool_name: tool_name.to_string(),
            content,
            is_error: false,
            details: None,
            timestamp: Utc::now(),
        })
    }

    #[test]
    fn summarize_bash_includes_command_and_exit_code() {
        let summary = summarize_tool_result(
            "bash",
            &json!({"command": "cargo test -p clankers-agent"}),
            &text_content("Exit code: 7\n\nline one\nline two"),
        );
        assert!(summary.contains("[bash] cargo test -p clankers-agent"));
        assert!(summary.contains("exit 7"));
        assert!(summary.contains("2 lines"));
    }

    #[test]
    fn summarize_read_includes_path_offset_and_char_count() {
        let summary =
            summarize_tool_result("read", &json!({"path": "src/main.rs", "offset": 42}), &text_content("hello world"));
        assert_eq!(summary, "[read] src/main.rs @42 (11 chars)");
    }

    #[test]
    fn summarize_write_counts_input_lines() {
        let summary = summarize_tool_result(
            "write",
            &json!({"path": "src/main.rs", "content": "a\nb\nc"}),
            &text_content("ignored output"),
        );
        assert_eq!(summary, "[write] src/main.rs (3 lines)");
    }

    #[test]
    fn summarize_grep_counts_matches() {
        let summary =
            summarize_tool_result("grep", &json!({"pattern": "TODO"}), &text_content("a.rs:1: TODO\nb.rs:2: TODO"));
        assert_eq!(summary, "[grep] pattern=TODO (2 matches)");
    }

    #[test]
    fn summarize_edit_counts_edits_array() {
        let summary = summarize_tool_result(
            "edit",
            &json!({"path": "src/main.rs", "edits": [{}, {}]}),
            &text_content("ignored output"),
        );
        assert_eq!(summary, "[edit] src/main.rs (2 edits)");
    }

    #[test]
    fn summarize_subagent_uses_goal_and_result_size() {
        let summary = summarize_tool_result(
            "subagent",
            &json!({"task": "Investigate why compaction does not trigger"}),
            &text_content("done"),
        );
        assert_eq!(summary, "[subagent] task=Investigate why compaction does not trigger (4 chars result)");
    }

    #[test]
    fn summarize_generic_fallback_uses_first_arg() {
        let summary = summarize_tool_result("custom", &json!({"alpha": "beta", "gamma": true}), &text_content("xyz"));
        assert_eq!(summary, "[custom] alpha=beta (3 chars result)");
    }

    #[test]
    fn prune_tool_results_uses_tool_call_arguments() {
        let call_id = "call-1";
        let messages = vec![
            assistant_tool_use(call_id, "read", json!({"path": "src/main.rs", "offset": 7})),
            tool_result(call_id, "read", text_content("hello")),
            tool_result("call-2", "write", text_content("recent output")),
        ];

        let pruned = prune_tool_results(&messages, 2);
        let AgentMessage::ToolResult(tool_result) = &pruned[1] else {
            panic!("expected tool result");
        };
        let Content::Text { text } = &tool_result.content[0] else {
            panic!("expected text content");
        };
        assert_eq!(text, "[read] src/main.rs @7 (5 chars)");
        assert!(existing_summary(tool_result).is_some());
    }

    #[test]
    fn prune_tool_results_preserves_existing_summary() {
        let call_id = "call-1";
        let summary = "[bash] ls (exit 0, 1 lines)";
        let messages = vec![tool_result(call_id, "bash", text_content(summary))];
        let AgentMessage::ToolResult(tool_result) = &messages[0] else {
            panic!("expected tool result");
        };
        let summarized = clone_with_summary(tool_result, summary.to_string());
        let pruned = prune_tool_results(&[AgentMessage::ToolResult(summarized.clone())], 1);
        let AgentMessage::ToolResult(pruned_result) = &pruned[0] else {
            panic!("expected tool result");
        };
        let Content::Text { text } = &pruned_result.content[0] else {
            panic!("expected text content");
        };
        assert_eq!(text, summary);
        assert_eq!(existing_summary(pruned_result), Some(summary));
    }

    #[test]
    fn summarize_generic_image_result_stays_single_line() {
        let summary = summarize_tool_result("screenshot", &Value::Null, &[Content::Image {
            source: ImageSource::Base64 {
                media_type: "image/png".to_string(),
                data: "abc".to_string(),
            },
        }]);
        assert_eq!(summary, "[screenshot] (image result)");
    }
}
