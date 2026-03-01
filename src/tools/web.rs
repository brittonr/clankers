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
                ctx.emit_progress("parsing results...");
                match response.json::<Value>().await {
                    Ok(json) => {
                        let results = format_search_results_streaming(&json, max_results, ctx);
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
                ctx.emit_progress(&format!("summarized: {} chars", output.len()));
                return ToolResult::text(format!(
                    "# Content from {}\n\n{}\n\n---\n*Summarized via Kagi Universal Summarizer*",
                    url, output
                ));
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
                    ctx.emit_progress(&format!("downloading: {} bytes", len));
                }

                match response.text().await {
                    Ok(body) => {
                        ctx.emit_progress(&format!("received: {} bytes, extracting text...", body.len()));
                        let clean = if content_type.contains("text/html") {
                            extract_text_from_html(&body)
                        } else {
                            body
                        };
                        // Truncate to avoid blowing context
                        let max_chars = 50_000;
                        if clean.len() > max_chars {
                            let truncated = &clean[..max_chars];
                            ctx.emit_progress(&format!("truncated: {} → {} chars", clean.len(), max_chars));
                            ToolResult::text(format!(
                                "# Content from {}\n\n{}...\n\n[Truncated: {} chars total]",
                                url,
                                truncated,
                                clean.len()
                            ))
                        } else {
                            ctx.emit_progress(&format!("done: {} chars", clean.len()));
                            ToolResult::text(format!("# Content from {}\n\n{}", url, clean))
                        }
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
        let max_results = params["max_results"].as_u64().unwrap_or(5) as usize;
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

            output.push_str(&format!("{}. **{}**\n", count + 1, title));
            output.push_str(&format!("   {}\n", url));
            if !snippet.is_empty() {
                output.push_str(&format!("   {}\n", snippet));
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
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;
    let mut last_was_space = false;

    let lower = html.to_lowercase();
    let chars: Vec<char> = html.chars().collect();
    let _lower_chars: Vec<char> = lower.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if !in_tag && chars[i] == '<' {
            in_tag = true;
            // Check for script/style start
            let remaining = &lower[i..];
            if remaining.starts_with("<script") {
                in_script = true;
            } else if remaining.starts_with("<style") {
                in_style = true;
            } else if remaining.starts_with("</script") {
                in_script = false;
            } else if remaining.starts_with("</style") {
                in_style = false;
            }
            // Block elements get newlines
            if (remaining.starts_with("<br")
                || remaining.starts_with("<p")
                || remaining.starts_with("<div")
                || remaining.starts_with("<h")
                || remaining.starts_with("<li")
                || remaining.starts_with("<tr"))
                && !result.ends_with('\n')
            {
                result.push('\n');
            }
            i += 1;
            continue;
        }
        if in_tag && chars[i] == '>' {
            in_tag = false;
            i += 1;
            continue;
        }
        if in_tag || in_script || in_style {
            i += 1;
            continue;
        }
        // Decode HTML entities
        if chars[i] == '&' {
            let rest = &lower[i..];
            if rest.starts_with("&amp;") {
                result.push('&');
                i += 5;
                last_was_space = false;
                continue;
            } else if rest.starts_with("&lt;") {
                result.push('<');
                i += 4;
                last_was_space = false;
                continue;
            } else if rest.starts_with("&gt;") {
                result.push('>');
                i += 4;
                last_was_space = false;
                continue;
            } else if rest.starts_with("&quot;") {
                result.push('"');
                i += 6;
                last_was_space = false;
                continue;
            } else if rest.starts_with("&nbsp;") {
                result.push(' ');
                i += 6;
                last_was_space = true;
                continue;
            } else if rest.starts_with("&#") {
                // Numeric entity
                if let Some(end) = rest.find(';') {
                    let num_str = &rest[2..end];
                    let code = if let Some(hex) = num_str.strip_prefix('x') {
                        u32::from_str_radix(hex, 16).ok()
                    } else {
                        num_str.parse::<u32>().ok()
                    };
                    if let Some(c) = code.and_then(char::from_u32) {
                        result.push(c);
                        i += end + 1;
                        last_was_space = false;
                        continue;
                    }
                }
            }
        }
        // Collapse whitespace
        if chars[i].is_whitespace() {
            if !last_was_space {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(chars[i]);
            last_was_space = false;
        }
        i += 1;
    }
    // Clean up excessive blank lines
    let mut clean = String::new();
    let mut blank_count = 0;
    for line in result.lines() {
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
