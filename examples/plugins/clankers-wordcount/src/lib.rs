//! clankers-wordcount — example plugin demonstrating how to build a clankers WASM plugin.
//!
//! This plugin provides two tools:
//!   - `wordcount`: basic word, line, and character counts
//!   - `textstats`: detailed text statistics (avg word length, top words, etc.)
//!
//! ## Building
//!
//! ```sh
//! cd examples/plugins/clankers-wordcount
//! cargo build --target wasm32-unknown-unknown --release -Zbuild-std=std,panic_abort
//! cp target/wasm32-unknown-unknown/release/clankers_wordcount.wasm .
//! ```
//!
//! ## Installing
//!
//! ```sh
//! clankers plugin install examples/plugins/clankers-wordcount
//! ```

use std::collections::HashMap;

use extism_pdk::*;
use serde::{Deserialize, Serialize};

// ── Tool call protocol ───────────────────────────────────────────────
//
// clankers sends tool calls as JSON:
//   { "tool": "<tool_name>", "args": { ... } }
//
// The plugin must return JSON:
//   { "tool": "<tool_name>", "result": "...", "status": "ok" | "error" }

#[derive(Deserialize)]
struct ToolCallInput {
    tool: String,
    args: serde_json::Value,
}

#[derive(Serialize)]
struct ToolCallOutput {
    tool: String,
    result: String,
    status: String,
}

/// Main entry point for tool calls. clankers routes tool invocations here
/// based on the `handler` field in plugin.json.
#[plugin_fn]
pub fn handle_tool_call(input: String) -> FnResult<String> {
    let call: ToolCallInput =
        serde_json::from_str(&input).map_err(|e| Error::msg(format!("Invalid JSON: {e}")))?;

    let text = call
        .args
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let (status, result) = match call.tool.as_str() {
        "wordcount" => ("ok", wordcount_impl(text)),
        "textstats" => {
            let top_n = call
                .args
                .get("top_n")
                .and_then(|v| v.as_u64())
                .unwrap_or(5) as usize;
            ("ok", textstats_impl(text, top_n))
        }
        _ => (
            "error",
            format!("Unknown tool: {}", call.tool),
        ),
    };

    let output = ToolCallOutput {
        tool: call.tool,
        result,
        status: status.to_string(),
    };
    Ok(serde_json::to_string(&output)?)
}

// ── Plugin metadata ──────────────────────────────────────────────────

#[derive(Serialize)]
struct PluginMeta {
    name: String,
    version: String,
    tools: Vec<ToolMeta>,
    commands: Vec<String>,
}

#[derive(Serialize)]
struct ToolMeta {
    name: String,
    description: String,
}

/// Return plugin metadata. Called by clankers during discovery if
/// `tool_definitions` is not present in plugin.json.
#[plugin_fn]
pub fn describe(Json(_): Json<()>) -> FnResult<Json<PluginMeta>> {
    Ok(Json(PluginMeta {
        name: "clankers-wordcount".to_string(),
        version: "0.1.0".to_string(),
        tools: vec![
            ToolMeta {
                name: "wordcount".to_string(),
                description: "Count words, lines, and characters in text".to_string(),
            },
            ToolMeta {
                name: "textstats".to_string(),
                description: "Detailed text statistics and frequency analysis".to_string(),
            },
        ],
        commands: vec![],
    }))
}

// ── Tool implementations ─────────────────────────────────────────────

#[derive(Serialize)]
struct WordCount {
    word_count: usize,
    line_count: usize,
    char_count: usize,
    byte_count: usize,
}

fn wordcount_impl(text: &str) -> String {
    let wc = WordCount {
        word_count: text.split_whitespace().count(),
        line_count: if text.is_empty() { 0 } else { text.lines().count() },
        char_count: text.chars().count(),
        byte_count: text.len(),
    };
    serde_json::to_string_pretty(&wc).unwrap_or_default()
}

#[derive(Serialize)]
struct TextStats {
    word_count: usize,
    unique_words: usize,
    avg_word_length: f64,
    longest_word: String,
    sentence_count: usize,
    paragraph_count: usize,
    reading_time_seconds: usize,
    top_words: Vec<WordFreq>,
}

#[derive(Serialize)]
struct WordFreq {
    word: String,
    count: usize,
}

fn textstats_impl(text: &str, top_n: usize) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let word_count = words.len();

    if word_count == 0 {
        let stats = TextStats {
            word_count: 0,
            unique_words: 0,
            avg_word_length: 0.0,
            longest_word: String::new(),
            sentence_count: 0,
            paragraph_count: 0,
            reading_time_seconds: 0,
            top_words: vec![],
        };
        return serde_json::to_string_pretty(&stats).unwrap_or_default();
    }

    // Normalize words for frequency counting (lowercase, strip punctuation)
    let mut freq: HashMap<String, usize> = HashMap::new();
    for w in &words {
        let normalized: String = w
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '\'')
            .collect::<String>()
            .to_lowercase();
        if !normalized.is_empty() {
            *freq.entry(normalized).or_insert(0) += 1;
        }
    }

    let unique_words = freq.len();

    let total_chars: usize = words.iter().map(|w| w.chars().filter(|c| c.is_alphanumeric()).count()).sum();
    let avg_word_length = total_chars as f64 / word_count as f64;

    let longest_word = words
        .iter()
        .max_by_key(|w| w.len())
        .unwrap_or(&"")
        .to_string();

    // Sentence count: split on .!? followed by space or end
    let sentence_count = text
        .chars()
        .filter(|c| *c == '.' || *c == '!' || *c == '?')
        .count()
        .max(1);

    // Paragraph count: split on double newlines
    let paragraph_count = text
        .split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .count()
        .max(1);

    // Average reading speed: ~250 words per minute
    let reading_time_seconds = (word_count as f64 / 250.0 * 60.0).ceil() as usize;

    // Top N words
    let mut sorted: Vec<_> = freq.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let top_words: Vec<WordFreq> = sorted
        .into_iter()
        .take(top_n)
        .map(|(word, count)| WordFreq { word, count })
        .collect();

    let stats = TextStats {
        word_count,
        unique_words,
        avg_word_length: (avg_word_length * 100.0).round() / 100.0,
        longest_word,
        sentence_count,
        paragraph_count,
        reading_time_seconds,
        top_words,
    };
    serde_json::to_string_pretty(&stats).unwrap_or_default()
}
