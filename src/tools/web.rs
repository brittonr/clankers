//! Web search and fetch tool — Kagi integration
//!
//! Provides two capabilities:
//! 1. **search** — Query the Kagi Search API and return summarized results
//! 2. **fetch** — Retrieve a URL's content as clean markdown (via Kagi Universal Summarizer or raw
//!    fetch)

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;
use super::progress::ResultChunk;
use super::progress::ToolProgress;

/// Environment variable for the Kagi API key
const KAGI_API_KEY_ENV: &str = "KAGI_API_KEY";
/// Kagi Search API endpoint
const KAGI_SEARCH_URL: &str = "https://kagi.com/api/v0/search";
/// Kagi Summarizer API endpoint
const KAGI_SUMMARIZER_URL: &str = "https://kagi.com/api/v0/summarize";

pub struct WebTool {
    definition: ToolDefinition,
}

impl Default for WebTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "web".to_string(),
                description: "Search the web or fetch a URL's content. Use action='search' to \
                    query the web with Kagi Search, or action='fetch' to retrieve and summarize \
                    a specific URL. Requires the KAGI_API_KEY environment variable."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["search", "fetch"],
                            "description": "Whether to search the web or fetch a specific URL"
                        },
                        "query": {
                            "type": "string",
                            "description": "Search query (for action='search') or URL (for action='fetch')"
                        },
                        "max_results": {
                            "type": "integer",
                            "description": "Maximum number of search results to return (default: 5, max: 20)",
                            "default": 5
                        }
                    },
                    "required": ["action", "query"]
                }),
            },
        }
    }

    fn get_api_key() -> Option<String> {
        std::env::var(KAGI_API_KEY_ENV).ok()
    }

    async fn search(&self, ctx: &ToolContext, query: &str, max_results: usize) -> ToolResult {
        let api_key = match Self::get_api_key() {
            Some(key) => key,
            None => {
                return ToolResult::error(format!(
                    "No Kagi API key found. Set the {} environment variable.",
                    KAGI_API_KEY_ENV
                ));
            }
        };

        ctx.emit_structured_progress(ToolProgress::phase("Searching", 1, Some(2)));
        ctx.emit_progress(&format!("querying Kagi: {}", query));

        let client = reqwest::Client::new();
        let resp = client
            .get(KAGI_SEARCH_URL)
            .header("Authorization", format!("Bot {}", api_key))
            .query(&[("q", query), ("limit", &max_results.to_string())])
            .send()
            .await;

        match resp {
            Ok(response) => {
                if !response.status().is_success() {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    return ToolResult::error(format!("Kagi search API error ({}): {}", status, body));
                }
                ctx.emit_structured_progress(ToolProgress::phase("Parsing results", 2, Some(2)));
                ctx.emit_progress("parsing results...");
                match response.json::<Value>().await {
                    Ok(json) => {
                        let results = format_search_results_streaming(&json, max_results, ctx);
                        ctx.emit_result_chunk(ResultChunk::text(&results));
                        ToolResult::text(results)
                    }
                    Err(e) => ToolResult::error(format!("Failed to parse Kagi response: {}", e)),
                }
            }
            Err(e) => ToolResult::error(format!("Kagi search request failed: {}", e)),
        }
    }

    async fn fetch(&self, ctx: &ToolContext, url: &str) -> ToolResult {
        // Try Kagi Summarizer first for clean content extraction
        ctx.emit_structured_progress(ToolProgress::phase("Fetching", 1, Some(2)));
        if let Some(api_key) = Self::get_api_key() {
            ctx.emit_progress(&format!("summarizing via Kagi: {}", url));
            let client = reqwest::Client::new();
            let resp = client
                .get(KAGI_SUMMARIZER_URL)
                .header("Authorization", format!("Bot {}", api_key))
                .query(&[("url", url), ("engine", "muriel"), ("summary_type", "takeaway")])
                .send()
                .await;

            if let Ok(response) = resp
                && response.status().is_success()
                && let Ok(json) = response.json::<Value>().await
                && let Some(output) = json["data"]["output"].as_str()
            {
                ctx.emit_structured_progress(ToolProgress::phase("Processing", 2, Some(2)));
                ctx.emit_progress(&format!("summarized: {} chars", output.len()));
                let result_text =
                    format!("# Content from {}\n\n{}\n\n---\n*Summarized via Kagi Universal Summarizer*", url, output);
                ctx.emit_result_chunk(ResultChunk::text(&result_text));
                return ToolResult::text(result_text);
            }
            ctx.emit_progress("Kagi summarizer unavailable, falling back to raw fetch");
        }

        // Fallback: raw HTTP fetch with basic content extraction
        ctx.emit_progress(&format!("fetching: {}", url));
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("clankers/0.1 (coding agent)")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        match client.get(url).send().await {
            Ok(response) => {
                if !response.status().is_success() {
                    return ToolResult::error(format!("HTTP {} fetching {}", response.status(), url));
                }
                let content_type =
                    response.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();

                let content_length = response.content_length();
                if let Some(len) = content_length {
                    ctx.emit_structured_progress(ToolProgress::bytes(0, Some(len)).with_message("Downloading"));
                    ctx.emit_progress(&format!("downloading: {} bytes", len));
                }

                match response.text().await {
                    Ok(body) => {
                        ctx.emit_structured_progress(ToolProgress::phase("Processing", 2, Some(2)));
                        ctx.emit_progress(&format!("received: {} bytes, extracting text...", body.len()));
                        let clean = if content_type.contains("text/html") {
                            extract_text_from_html(&body)
                        } else {
                            body
                        };
                        // Truncate to avoid blowing context
                        let max_chars = 50_000;
                        let result_text = if clean.len() > max_chars {
                            let truncated = &clean[..max_chars];
                            ctx.emit_progress(&format!("truncated: {} → {} chars", clean.len(), max_chars));
                            format!(
                                "# Content from {}\n\n{}...\n\n[Truncated: {} chars total]",
                                url,
                                truncated,
                                clean.len()
                            )
                        } else {
                            ctx.emit_progress(&format!("done: {} chars", clean.len()));
                            format!("# Content from {}\n\n{}", url, clean)
                        };
                        ctx.emit_result_chunk(ResultChunk::text(&result_text));
                        ToolResult::text(result_text)
                    }
                    Err(e) => ToolResult::error(format!("Failed to read response body: {}", e)),
                }
            }
            Err(e) => ToolResult::error(format!("Failed to fetch {}: {}", url, e)),
        }
    }
}

#[async_trait]
impl Tool for WebTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("search");
        let query = match params["query"].as_str() {
            Some(q) => q,
            None => return ToolResult::error("Missing required parameter: query"),
        };
        let max_results = usize::try_from(params["max_results"].as_u64().unwrap_or(5)).unwrap_or(5);
        let max_results = max_results.min(20);

        match action {
            "search" => self.search(ctx, query, max_results).await,
            "fetch" => self.fetch(ctx, query).await,
            _ => ToolResult::error(format!("Unknown action: {}. Use 'search' or 'fetch'.", action)),
        }
    }
}

/// Format Kagi search results, streaming each result as it's processed
fn format_search_results_streaming(json: &Value, max: usize, ctx: &ToolContext) -> String {
    use std::fmt::Write;
    let mut output = String::new();
    if let Some(results) = json["data"].as_array() {
        let mut count = 0;
        for result in results {
            if count >= max {
                break;
            }
            let t = result["t"].as_u64().unwrap_or(0);
            if t != 0 {
                continue;
            }

            let title = result["title"].as_str().unwrap_or("(no title)");
            let url = result["url"].as_str().unwrap_or("");
            let snippet = result["snippet"].as_str().unwrap_or("");

            // Stream each result as it's formatted
            ctx.emit_progress(&format!("{}. {}", count + 1, title));

            writeln!(output, "{}. **{}**", count + 1, title).ok();
            writeln!(output, "   {}", url).ok();
            if !snippet.is_empty() {
                writeln!(output, "   {}", snippet).ok();
            }
            output.push('\n');
            count += 1;
        }
        if count == 0 {
            output.push_str("No results found.\n");
        }
    } else {
        output.push_str("No results found.\n");
    }
    output
}

/// Very basic HTML text extraction (strip tags, decode common entities)
fn extract_text_from_html(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut is_in_tag = false;
    let mut is_in_script = false;
    let mut is_in_style = false;
    let mut was_last_space = false;

    let lower = html.to_lowercase();
    let chars: Vec<char> = html.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Handle tag opening
        if !is_in_tag && chars[i] == '<' {
            let skip = handle_tag_open(&lower[i..], &mut is_in_tag, &mut is_in_script, &mut is_in_style, &mut result);
            i += skip;
            continue;
        }

        // Handle tag closing
        if is_in_tag && chars[i] == '>' {
            is_in_tag = false;
            i += 1;
            continue;
        }

        // Skip content inside tags or script/style blocks
        if is_in_tag || is_in_script || is_in_style {
            i += 1;
            continue;
        }

        // Decode HTML entities
        if chars[i] == '&'
            && let Some((ch, skip)) = decode_html_entity(&lower[i..])
        {
            result.push(ch);
            i += skip;
            was_last_space = ch == ' ';
            continue;
        }

        // Collapse whitespace
        handle_text_char(chars[i], &mut result, &mut was_last_space);
        i += 1;
    }

    normalize_blank_lines(&result)
}

/// Handle opening tag: detect script/style blocks and insert newlines for block elements
fn handle_tag_open(
    remaining_lower: &str,
    is_in_tag: &mut bool,
    is_in_script: &mut bool,
    is_in_style: &mut bool,
    result: &mut String,
) -> usize {
    *is_in_tag = true;

    // Track script/style blocks
    if remaining_lower.starts_with("<script") {
        *is_in_script = true;
    } else if remaining_lower.starts_with("<style") {
        *is_in_style = true;
    } else if remaining_lower.starts_with("</script") {
        *is_in_script = false;
    } else if remaining_lower.starts_with("</style") {
        *is_in_style = false;
    }

    // Block elements get newlines
    let is_block = remaining_lower.starts_with("<br")
        || remaining_lower.starts_with("<p")
        || remaining_lower.starts_with("<div")
        || remaining_lower.starts_with("<h")
        || remaining_lower.starts_with("<li")
        || remaining_lower.starts_with("<tr");

    if is_block && !result.ends_with('\n') {
        result.push('\n');
    }

    1 // Skip the '<' character
}

/// Decode a single HTML entity if present. Returns (decoded char, bytes to skip) or None.
#[cfg_attr(dylint_lib = "tigerstyle", allow(nested_conditionals, reason = "complex control flow — extracting helpers would obscure logic"))]
fn decode_html_entity(text_lower: &str) -> Option<(char, usize)> {
    if text_lower.starts_with("&amp;") {
        Some(('&', 5))
    } else if text_lower.starts_with("&lt;") {
        Some(('<', 4))
    } else if text_lower.starts_with("&gt;") {
        Some(('>', 4))
    } else if text_lower.starts_with("&quot;") {
        Some(('"', 6))
    } else if text_lower.starts_with("&nbsp;") {
        Some((' ', 6))
    } else if text_lower.starts_with("&#") {
        // Numeric entity: &#123; or &#xAB;
        text_lower.find(';').and_then(|end| {
            let num_str = &text_lower[2..end];
            let code = if let Some(hex) = num_str.strip_prefix('x') {
                u32::from_str_radix(hex, 16).ok()
            } else {
                num_str.parse::<u32>().ok()
            };
            code.and_then(char::from_u32).map(|c| (c, end + 1))
        })
    } else {
        None
    }
}

/// Handle a single text character: collapse whitespace and push to result
fn handle_text_char(ch: char, result: &mut String, was_last_space: &mut bool) {
    if ch.is_whitespace() {
        if !*was_last_space {
            result.push(' ');
            *was_last_space = true;
        }
    } else {
        result.push(ch);
        *was_last_space = false;
    }
}

/// Normalize excessive blank lines (max 2 consecutive)
fn normalize_blank_lines(text: &str) -> String {
    let mut clean = String::new();
    let mut blank_count = 0;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                clean.push('\n');
            }
        } else {
            blank_count = 0;
            clean.push_str(trimmed);
            clean.push('\n');
        }
    }
    clean
}

#[cfg(test)]
mod tests {
    use tokio_util::sync::CancellationToken;

    use super::*;

    #[test]
    fn test_extract_text_from_html() {
        let html = "<html><body><h1>Hello</h1><p>World &amp; friends</p></body></html>";
        let text = extract_text_from_html(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("World & friends"));
    }

    #[test]
    fn test_extract_strips_scripts() {
        let html = "<div>Before</div><script>var x=1;</script><div>After</div>";
        let text = extract_text_from_html(html);
        assert!(text.contains("Before"));
        assert!(text.contains("After"));
        assert!(!text.contains("var x"));
    }

    #[test]
    fn test_format_search_results_empty() {
        let ctx = ToolContext::new("test".to_string(), CancellationToken::new(), None);
        let json = json!({"data": []});
        let result = format_search_results_streaming(&json, 5, &ctx);
        assert!(result.contains("No results"));
    }

    #[test]
    fn test_format_search_results() {
        let ctx = ToolContext::new("test".to_string(), CancellationToken::new(), None);
        let json = json!({
            "data": [
                {"t": 0, "title": "Rust Lang", "url": "https://rust-lang.org", "snippet": "A systems language"},
                {"t": 0, "title": "Cargo", "url": "https://crates.io", "snippet": "Package manager"}
            ]
        });
        let result = format_search_results_streaming(&json, 5, &ctx);
        assert!(result.contains("Rust Lang"));
        assert!(result.contains("Cargo"));
    }

    #[test]
    fn test_web_tool_definition() {
        let tool = WebTool::new();
        assert_eq!(tool.definition().name, "web");
    }
}
