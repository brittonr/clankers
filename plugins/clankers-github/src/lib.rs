//! clankers-github plugin — GitHub integration for clankers.
//!
//! Provides tools for interacting with GitHub's REST API:
//!
//! - **`github_pr_list`** — List pull requests
//! - **`github_pr_get`** — Get PR details (diff stats, reviews, merge state)
//! - **`github_pr_create`** — Create a new pull request
//! - **`github_issues`** — List issues
//! - **`github_issue_get`** — Get issue details
//! - **`github_actions_status`** — CI/CD workflow run status
//! - **`github_repo_info`** — Repository metadata
//!
//! Requires `GITHUB_TOKEN` environment variable for authentication.

use clankers_plugin_sdk::prelude::*;
use serde_json::json;

// ═══════════════════════════════════════════════════════════════════════
//  Constants
// ═══════════════════════════════════════════════════════════════════════

const API_BASE: &str = "https://api.github.com";
const USER_AGENT: &str = "clankers-github-plugin/0.1.0";

// ═══════════════════════════════════════════════════════════════════════
//  Extism guest functions
// ═══════════════════════════════════════════════════════════════════════

/// Dispatch a tool call to the appropriate handler.
#[plugin_fn]
pub fn handle_tool_call(input: String) -> FnResult<String> {
    dispatch_tools(&input, &[
        ("github_pr_list", handle_pr_list),
        ("github_pr_get", handle_pr_get),
        ("github_pr_create", handle_pr_create),
        ("github_issues", handle_issues),
        ("github_issue_get", handle_issue_get),
        ("github_actions_status", handle_actions_status),
        ("github_repo_info", handle_repo_info),
    ])
}

/// Handle a plugin lifecycle event.
#[plugin_fn]
pub fn on_event(input: String) -> FnResult<String> {
    dispatch_events(&input, "clankers-github", &[
        ("agent_start", |_| "clankers-github plugin ready".to_string()),
        ("agent_end", |_| "clankers-github plugin shutting down".to_string()),
    ])
}

/// Return plugin metadata as JSON.
#[plugin_fn]
pub fn describe(Json(_): Json<()>) -> FnResult<Json<PluginMeta>> {
    Ok(Json(PluginMeta::new("clankers-github", "0.1.0", &[
        ("github_pr_list", "List pull requests for a GitHub repository"),
        ("github_pr_get", "Get detailed PR info (diff stats, reviews, merge state)"),
        ("github_pr_create", "Create a new pull request"),
        ("github_issues", "List issues for a GitHub repository"),
        ("github_issue_get", "Get detailed issue info"),
        ("github_actions_status", "Get CI/CD workflow run status"),
        ("github_repo_info", "Get repository metadata"),
    ], &[])))
}

// ═══════════════════════════════════════════════════════════════════════
//  GitHub API helpers
// ═══════════════════════════════════════════════════════════════════════

/// Get the GitHub token from plugin config (injected by host from GITHUB_TOKEN env).
fn get_token() -> Result<String, String> {
    extism_pdk::config::get("github_token")
        .map_err(|e| format!("Failed to read config: {e}"))?
        .ok_or_else(|| {
            "GITHUB_TOKEN not configured. Set the GITHUB_TOKEN environment variable.".to_string()
        })
}

/// Make an authenticated GET request to the GitHub API.
fn github_get(path: &str) -> Result<Value, String> {
    let token = get_token()?;
    let url = if path.starts_with("https://") {
        path.to_string()
    } else {
        format!("{API_BASE}{path}")
    };

    let req = extism_pdk::HttpRequest::new(&url)
        .with_header("Authorization", &format!("Bearer {token}"))
        .with_header("Accept", "application/vnd.github+json")
        .with_header("User-Agent", USER_AGENT)
        .with_header("X-GitHub-Api-Version", "2022-11-28")
        .with_method("GET");

    let resp = extism_pdk::http::request::<()>(&req, None)
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let body = resp.body();
    let text = String::from_utf8_lossy(&body);

    let status = resp.status_code();
    if status < 200 || status >= 300 {
        // Try to extract GitHub error message
        if let Ok(err_json) = serde_json::from_str::<Value>(&text) {
            let msg = err_json.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            return Err(format!("GitHub API error (HTTP {status}): {msg}"));
        }
        return Err(format!("GitHub API error (HTTP {status}): {text}"));
    }

    serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse GitHub response: {e}"))
}

/// Make an authenticated POST request to the GitHub API.
fn github_post(path: &str, body: &Value) -> Result<Value, String> {
    let token = get_token()?;
    let url = format!("{API_BASE}{path}");

    let body_bytes = serde_json::to_vec(body)
        .map_err(|e| format!("Failed to serialize request body: {e}"))?;

    let req = extism_pdk::HttpRequest::new(&url)
        .with_header("Authorization", &format!("Bearer {token}"))
        .with_header("Accept", "application/vnd.github+json")
        .with_header("Content-Type", "application/json")
        .with_header("User-Agent", USER_AGENT)
        .with_header("X-GitHub-Api-Version", "2022-11-28")
        .with_method("POST");

    let resp = extism_pdk::http::request(&req, Some(body_bytes))
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let resp_body = resp.body();
    let text = String::from_utf8_lossy(&resp_body);

    let status = resp.status_code();
    if status < 200 || status >= 300 {
        if let Ok(err_json) = serde_json::from_str::<Value>(&text) {
            let msg = err_json.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            let errors = err_json.get("errors")
                .map(|e| format!(" Details: {e}"))
                .unwrap_or_default();
            return Err(format!("GitHub API error (HTTP {status}): {msg}{errors}"));
        }
        return Err(format!("GitHub API error (HTTP {status}): {text}"));
    }

    serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse GitHub response: {e}"))
}

/// Format a GitHub timestamp (ISO 8601) into a human-readable relative time.
fn format_time(iso: &str) -> String {
    // Parse the host-injected current_time for comparison
    let now_unix = extism_pdk::config::get("current_time_unix")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    if now_unix == 0 {
        // Can't compute relative time, return the raw timestamp
        return iso.to_string();
    }

    // Parse ISO 8601 timestamp manually (YYYY-MM-DDTHH:MM:SSZ)
    let ts_unix = parse_iso8601_unix(iso).unwrap_or(0);
    if ts_unix == 0 {
        return iso.to_string();
    }

    let delta = now_unix - ts_unix;
    if delta < 0 {
        return "just now".to_string();
    }
    let delta = delta as u64;

    match delta {
        0..=59 => "just now".to_string(),
        60..=3599 => format!("{}m ago", delta / 60),
        3600..=86399 => format!("{}h ago", delta / 3600),
        86400..=2591999 => format!("{}d ago", delta / 86400),
        _ => format!("{}mo ago", delta / 2592000),
    }
}

/// Parse an ISO 8601 timestamp to Unix seconds.
/// Handles "YYYY-MM-DDTHH:MM:SSZ" format.
fn parse_iso8601_unix(iso: &str) -> Option<i64> {
    // Minimal parser for GitHub's timestamp format
    let s = iso.trim_end_matches('Z');
    let parts: Vec<&str> = s.split('T').collect();
    if parts.len() != 2 {
        return None;
    }

    let date_parts: Vec<u32> = parts[0].split('-')
        .filter_map(|p| p.parse().ok())
        .collect();
    let time_parts: Vec<u32> = parts[1].split(':')
        .filter_map(|p| p.parse().ok())
        .collect();

    if date_parts.len() != 3 || time_parts.len() != 3 {
        return None;
    }

    let (year, month, day) = (date_parts[0] as i64, date_parts[1] as i64, date_parts[2] as i64);
    let (hour, min, sec) = (time_parts[0] as i64, time_parts[1] as i64, time_parts[2] as i64);

    // Days from year 1970 to the given year (simplified, ignoring leap second details)
    let mut days: i64 = 0;
    for y in 1970..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }

    let month_days = [31, if is_leap_year(year) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 0..(month - 1) as usize {
        if m < 12 {
            days += month_days[m] as i64;
        }
    }
    days += day - 1;

    Some(days * 86400 + hour * 3600 + min * 60 + sec)
}

fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Truncate a string to max chars, appending "…" if truncated.
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Tool implementations
// ═══════════════════════════════════════════════════════════════════════

/// List pull requests.
fn handle_pr_list(args: &Value) -> Result<String, String> {
    let owner = args.require_str("owner")?;
    let repo = args.require_str("repo")?;
    let state = args.get_str_or("state", "open");
    let per_page = args.get_u64_or("per_page", 10).min(100);
    let page = args.get_u64_or("page", 1).max(1);

    let path = format!(
        "/repos/{owner}/{repo}/pulls?state={state}&per_page={per_page}&page={page}&sort=updated&direction=desc"
    );
    let data = github_get(&path)?;

    let prs = data.as_array().ok_or("Expected array response")?;
    if prs.is_empty() {
        return Ok(format!("No {state} pull requests found for {owner}/{repo}."));
    }

    let mut out = format!("## Pull Requests — {owner}/{repo} ({state})\n\n");

    for pr in prs {
        let number = pr.get("number").and_then(|v| v.as_u64()).unwrap_or(0);
        let title = pr.get("title").and_then(|v| v.as_str()).unwrap_or("(no title)");
        let author = pr.get("user")
            .and_then(|u| u.get("login"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let state_str = pr.get("state").and_then(|v| v.as_str()).unwrap_or("unknown");
        let draft = pr.get("draft").and_then(|v| v.as_bool()).unwrap_or(false);
        let updated = pr.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");
        let labels: Vec<&str> = pr.get("labels")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter()
                .filter_map(|l| l.get("name").and_then(|v| v.as_str()))
                .collect())
            .unwrap_or_default();

        let draft_marker = if draft { " [DRAFT]" } else { "" };
        let label_str = if labels.is_empty() {
            String::new()
        } else {
            format!(" [{}]", labels.join(", "))
        };

        out.push_str(&format!(
            "- **#{number}** {}{draft_marker} — by @{author} ({state_str}, {}){label_str}\n",
            truncate(title, 80),
            format_time(updated),
        ));
    }

    Ok(out)
}

/// Get detailed PR info.
fn handle_pr_get(args: &Value) -> Result<String, String> {
    let owner = args.require_str("owner")?;
    let repo = args.require_str("repo")?;
    let number = args.get_u64("number")
        .ok_or("missing required parameter: number")?;

    let pr = github_get(&format!("/repos/{owner}/{repo}/pulls/{number}"))?;

    let title = pr.get("title").and_then(|v| v.as_str()).unwrap_or("(no title)");
    let body = pr.get("body").and_then(|v| v.as_str()).unwrap_or("(no description)");
    let state = pr.get("state").and_then(|v| v.as_str()).unwrap_or("unknown");
    let draft = pr.get("draft").and_then(|v| v.as_bool()).unwrap_or(false);
    let author = pr.get("user")
        .and_then(|u| u.get("login"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let head_ref = pr.get("head")
        .and_then(|h| h.get("ref"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let base_ref = pr.get("base")
        .and_then(|b| b.get("ref"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let mergeable = pr.get("mergeable").and_then(|v| v.as_bool());
    let merged = pr.get("merged").and_then(|v| v.as_bool()).unwrap_or(false);
    let additions = pr.get("additions").and_then(|v| v.as_u64()).unwrap_or(0);
    let deletions = pr.get("deletions").and_then(|v| v.as_u64()).unwrap_or(0);
    let changed_files = pr.get("changed_files").and_then(|v| v.as_u64()).unwrap_or(0);
    let comments = pr.get("comments").and_then(|v| v.as_u64()).unwrap_or(0);
    let review_comments = pr.get("review_comments").and_then(|v| v.as_u64()).unwrap_or(0);
    let created = pr.get("created_at").and_then(|v| v.as_str()).unwrap_or("");
    let updated = pr.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");
    let html_url = pr.get("html_url").and_then(|v| v.as_str()).unwrap_or("");

    let labels: Vec<&str> = pr.get("labels")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter()
            .filter_map(|l| l.get("name").and_then(|v| v.as_str()))
            .collect())
        .unwrap_or_default();

    let reviewers: Vec<&str> = pr.get("requested_reviewers")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter()
            .filter_map(|r| r.get("login").and_then(|v| v.as_str()))
            .collect())
        .unwrap_or_default();

    let assignees: Vec<&str> = pr.get("assignees")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter()
            .filter_map(|a| a.get("login").and_then(|v| v.as_str()))
            .collect())
        .unwrap_or_default();

    let draft_str = if draft { " (DRAFT)" } else { "" };
    let merge_status = if merged {
        "✅ Merged".to_string()
    } else {
        match mergeable {
            Some(true) => "🟢 Mergeable".to_string(),
            Some(false) => "🔴 Conflicts".to_string(),
            None => "⏳ Checking…".to_string(),
        }
    };

    let mut out = format!("## PR #{number}: {title}{draft_str}\n\n");
    out.push_str(&format!("**State:** {state} | {merge_status}\n"));
    out.push_str(&format!("**Branch:** `{head_ref}` → `{base_ref}`\n"));
    out.push_str(&format!("**Author:** @{author}\n"));
    out.push_str(&format!("**Created:** {} | **Updated:** {}\n", format_time(created), format_time(updated)));
    out.push_str(&format!("**Diff:** +{additions} −{deletions} across {changed_files} files\n"));
    out.push_str(&format!("**Discussion:** {comments} comments, {review_comments} review comments\n"));

    if !labels.is_empty() {
        out.push_str(&format!("**Labels:** {}\n", labels.join(", ")));
    }
    if !reviewers.is_empty() {
        out.push_str(&format!("**Reviewers:** {}\n", reviewers.iter().map(|r| format!("@{r}")).collect::<Vec<_>>().join(", ")));
    }
    if !assignees.is_empty() {
        out.push_str(&format!("**Assignees:** {}\n", assignees.iter().map(|a| format!("@{a}")).collect::<Vec<_>>().join(", ")));
    }

    out.push_str(&format!("**URL:** {html_url}\n"));

    // Truncate body if very long
    let body_display = if body.len() > 2000 {
        format!("{}…\n\n_(truncated, {} chars total)_", &body[..2000], body.len())
    } else {
        body.to_string()
    };

    out.push_str(&format!("\n### Description\n\n{body_display}\n"));

    // Fetch reviews
    if let Ok(reviews) = github_get(&format!("/repos/{owner}/{repo}/pulls/{number}/reviews")) {
        if let Some(reviews_arr) = reviews.as_array() {
            if !reviews_arr.is_empty() {
                out.push_str("\n### Reviews\n\n");
                for review in reviews_arr {
                    let reviewer = review.get("user")
                        .and_then(|u| u.get("login"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let review_state = review.get("state")
                        .and_then(|v| v.as_str())
                        .unwrap_or("PENDING");
                    let icon = match review_state {
                        "APPROVED" => "✅",
                        "CHANGES_REQUESTED" => "🔴",
                        "COMMENTED" => "💬",
                        "DISMISSED" => "🚫",
                        _ => "⏳",
                    };
                    out.push_str(&format!("- {icon} @{reviewer}: {review_state}\n"));
                }
            }
        }
    }

    Ok(out)
}

/// Create a new pull request.
fn handle_pr_create(args: &Value) -> Result<String, String> {
    let owner = args.require_str("owner")?;
    let repo = args.require_str("repo")?;
    let title = args.require_str("title")?;
    let head = args.require_str("head")?;
    let base = args.get_str_or("base", "main");
    let body = args.get_str_or("body", "");
    let draft = args.get_bool_or("draft", false);

    let payload = json!({
        "title": title,
        "head": head,
        "base": base,
        "body": body,
        "draft": draft,
    });

    let result = github_post(&format!("/repos/{owner}/{repo}/pulls"), &payload)?;

    let number = result.get("number").and_then(|v| v.as_u64()).unwrap_or(0);
    let html_url = result.get("html_url").and_then(|v| v.as_str()).unwrap_or("");

    let draft_str = if draft { " (draft)" } else { "" };
    Ok(format!(
        "✅ Created PR #{number}{draft_str}: {title}\n\n\
         **Branch:** `{head}` → `{base}`\n\
         **URL:** {html_url}"
    ))
}

/// List issues.
fn handle_issues(args: &Value) -> Result<String, String> {
    let owner = args.require_str("owner")?;
    let repo = args.require_str("repo")?;
    let state = args.get_str_or("state", "open");
    let per_page = args.get_u64_or("per_page", 10).min(100);
    let page = args.get_u64_or("page", 1).max(1);
    let labels = args.get_str("labels").unwrap_or("");

    let mut path = format!(
        "/repos/{owner}/{repo}/issues?state={state}&per_page={per_page}&page={page}&sort=updated&direction=desc"
    );
    if !labels.is_empty() {
        path.push_str(&format!("&labels={labels}"));
    }

    let data = github_get(&path)?;
    let issues = data.as_array().ok_or("Expected array response")?;

    // Filter out PRs (GitHub includes PRs in the issues endpoint)
    let issues: Vec<&Value> = issues.iter()
        .filter(|i| i.get("pull_request").is_none())
        .collect();

    if issues.is_empty() {
        return Ok(format!("No {state} issues found for {owner}/{repo}."));
    }

    let mut out = format!("## Issues — {owner}/{repo} ({state})\n\n");

    for issue in &issues {
        let number = issue.get("number").and_then(|v| v.as_u64()).unwrap_or(0);
        let title = issue.get("title").and_then(|v| v.as_str()).unwrap_or("(no title)");
        let author = issue.get("user")
            .and_then(|u| u.get("login"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let comments = issue.get("comments").and_then(|v| v.as_u64()).unwrap_or(0);
        let updated = issue.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");
        let labels: Vec<&str> = issue.get("labels")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter()
                .filter_map(|l| l.get("name").and_then(|v| v.as_str()))
                .collect())
            .unwrap_or_default();

        let comment_str = if comments > 0 { format!(" 💬{comments}") } else { String::new() };
        let label_str = if labels.is_empty() {
            String::new()
        } else {
            format!(" [{}]", labels.join(", "))
        };

        out.push_str(&format!(
            "- **#{number}** {} — by @{author} ({}){comment_str}{label_str}\n",
            truncate(title, 80),
            format_time(updated),
        ));
    }

    Ok(out)
}

/// Get detailed issue info.
fn handle_issue_get(args: &Value) -> Result<String, String> {
    let owner = args.require_str("owner")?;
    let repo = args.require_str("repo")?;
    let number = args.get_u64("number")
        .ok_or("missing required parameter: number")?;

    let issue = github_get(&format!("/repos/{owner}/{repo}/issues/{number}"))?;

    // Check if this is actually a PR
    if issue.get("pull_request").is_some() {
        return Err(format!("#{number} is a pull request, not an issue. Use github_pr_get instead."));
    }

    let title = issue.get("title").and_then(|v| v.as_str()).unwrap_or("(no title)");
    let body = issue.get("body").and_then(|v| v.as_str()).unwrap_or("(no description)");
    let state = issue.get("state").and_then(|v| v.as_str()).unwrap_or("unknown");
    let author = issue.get("user")
        .and_then(|u| u.get("login"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let created = issue.get("created_at").and_then(|v| v.as_str()).unwrap_or("");
    let updated = issue.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");
    let comments = issue.get("comments").and_then(|v| v.as_u64()).unwrap_or(0);
    let html_url = issue.get("html_url").and_then(|v| v.as_str()).unwrap_or("");

    let labels: Vec<&str> = issue.get("labels")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter()
            .filter_map(|l| l.get("name").and_then(|v| v.as_str()))
            .collect())
        .unwrap_or_default();

    let assignees: Vec<&str> = issue.get("assignees")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter()
            .filter_map(|a| a.get("login").and_then(|v| v.as_str()))
            .collect())
        .unwrap_or_default();

    let milestone = issue.get("milestone")
        .and_then(|m| m.get("title"))
        .and_then(|v| v.as_str());

    let state_icon = match state {
        "open" => "🟢",
        "closed" => "🔴",
        _ => "⚪",
    };

    let mut out = format!("## Issue #{number}: {title}\n\n");
    out.push_str(&format!("**State:** {state_icon} {state}\n"));
    out.push_str(&format!("**Author:** @{author}\n"));
    out.push_str(&format!("**Created:** {} | **Updated:** {}\n", format_time(created), format_time(updated)));
    out.push_str(&format!("**Comments:** {comments}\n"));

    if !labels.is_empty() {
        out.push_str(&format!("**Labels:** {}\n", labels.join(", ")));
    }
    if !assignees.is_empty() {
        out.push_str(&format!("**Assignees:** {}\n", assignees.iter().map(|a| format!("@{a}")).collect::<Vec<_>>().join(", ")));
    }
    if let Some(ms) = milestone {
        out.push_str(&format!("**Milestone:** {ms}\n"));
    }

    out.push_str(&format!("**URL:** {html_url}\n"));

    let body_display = if body.len() > 3000 {
        format!("{}…\n\n_(truncated, {} chars total)_", &body[..3000], body.len())
    } else {
        body.to_string()
    };

    out.push_str(&format!("\n### Description\n\n{body_display}\n"));

    // Fetch recent comments
    if comments > 0 {
        if let Ok(comments_data) = github_get(&format!("/repos/{owner}/{repo}/issues/{number}/comments?per_page=5&direction=desc")) {
            if let Some(comments_arr) = comments_data.as_array() {
                if !comments_arr.is_empty() {
                    out.push_str("\n### Recent Comments\n\n");
                    for comment in comments_arr {
                        let c_author = comment.get("user")
                            .and_then(|u| u.get("login"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        let c_created = comment.get("created_at")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let c_body = comment.get("body")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let c_body_display = truncate(c_body, 500);
                        out.push_str(&format!(
                            "**@{c_author}** ({})\n{c_body_display}\n\n",
                            format_time(c_created),
                        ));
                    }
                }
            }
        }
    }

    Ok(out)
}

/// Get GitHub Actions workflow run status.
fn handle_actions_status(args: &Value) -> Result<String, String> {
    let owner = args.require_str("owner")?;
    let repo = args.require_str("repo")?;
    let git_ref = args.get_str_or("ref", "main");
    let per_page = args.get_u64_or("per_page", 5).min(20);

    let path = format!(
        "/repos/{owner}/{repo}/actions/runs?branch={git_ref}&per_page={per_page}"
    );
    let data = github_get(&path)?;

    let runs = data.get("workflow_runs")
        .and_then(|v| v.as_array())
        .ok_or("Expected workflow_runs array")?;

    if runs.is_empty() {
        return Ok(format!("No workflow runs found for {owner}/{repo} on `{git_ref}`."));
    }

    let total_count = data.get("total_count").and_then(|v| v.as_u64()).unwrap_or(0);

    let mut out = format!("## CI Status — {owner}/{repo} (`{git_ref}`)\n\n");
    out.push_str(&format!("_{total_count} total runs_\n\n"));

    for run in runs {
        let name = run.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        let status = run.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
        let conclusion = run.get("conclusion").and_then(|v| v.as_str());
        let run_number = run.get("run_number").and_then(|v| v.as_u64()).unwrap_or(0);
        let created = run.get("created_at").and_then(|v| v.as_str()).unwrap_or("");
        let updated = run.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");
        let html_url = run.get("html_url").and_then(|v| v.as_str()).unwrap_or("");
        let event = run.get("event").and_then(|v| v.as_str()).unwrap_or("?");
        let head_sha = run.get("head_sha").and_then(|v| v.as_str()).unwrap_or("?");

        let icon = match conclusion {
            Some("success") => "✅",
            Some("failure") => "❌",
            Some("cancelled") => "🚫",
            Some("skipped") => "⏭️",
            Some("timed_out") => "⏰",
            _ => match status {
                "in_progress" => "🔄",
                "queued" => "⏳",
                "waiting" => "⏳",
                _ => "❓",
            },
        };

        let conclusion_str = conclusion.unwrap_or(status);
        let sha_short = if head_sha.len() > 7 { &head_sha[..7] } else { head_sha };

        out.push_str(&format!(
            "- {icon} **{name}** #{run_number} — {conclusion_str} ({event} on `{sha_short}`, {})\n  {html_url}\n",
            format_time(updated),
        ));

        // Show duration if both timestamps are available
        if !created.is_empty() && !updated.is_empty() {
            if let (Some(start), Some(end)) = (parse_iso8601_unix(created), parse_iso8601_unix(updated)) {
                let duration_secs = (end - start).max(0) as u64;
                if duration_secs > 0 && conclusion.is_some() {
                    let duration_str = if duration_secs >= 3600 {
                        format!("{}h {}m", duration_secs / 3600, (duration_secs % 3600) / 60)
                    } else if duration_secs >= 60 {
                        format!("{}m {}s", duration_secs / 60, duration_secs % 60)
                    } else {
                        format!("{duration_secs}s")
                    };
                    out.push_str(&format!("  ⏱️ {duration_str}\n"));
                }
            }
        }
    }

    Ok(out)
}

/// Get repository metadata.
fn handle_repo_info(args: &Value) -> Result<String, String> {
    let owner = args.require_str("owner")?;
    let repo = args.require_str("repo")?;

    let data = github_get(&format!("/repos/{owner}/{repo}"))?;

    let full_name = data.get("full_name").and_then(|v| v.as_str()).unwrap_or("?");
    let description = data.get("description").and_then(|v| v.as_str()).unwrap_or("(no description)");
    let language = data.get("language").and_then(|v| v.as_str()).unwrap_or("unknown");
    let stars = data.get("stargazers_count").and_then(|v| v.as_u64()).unwrap_or(0);
    let forks = data.get("forks_count").and_then(|v| v.as_u64()).unwrap_or(0);
    let open_issues = data.get("open_issues_count").and_then(|v| v.as_u64()).unwrap_or(0);
    let watchers = data.get("subscribers_count").and_then(|v| v.as_u64()).unwrap_or(0);
    let default_branch = data.get("default_branch").and_then(|v| v.as_str()).unwrap_or("main");
    let is_fork = data.get("fork").and_then(|v| v.as_bool()).unwrap_or(false);
    let is_archived = data.get("archived").and_then(|v| v.as_bool()).unwrap_or(false);
    let is_private = data.get("private").and_then(|v| v.as_bool()).unwrap_or(false);
    let created = data.get("created_at").and_then(|v| v.as_str()).unwrap_or("");
    let updated = data.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");
    let pushed = data.get("pushed_at").and_then(|v| v.as_str()).unwrap_or("");
    let html_url = data.get("html_url").and_then(|v| v.as_str()).unwrap_or("");
    let size_kb = data.get("size").and_then(|v| v.as_u64()).unwrap_or(0);

    let license = data.get("license")
        .and_then(|l| l.get("spdx_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("None");

    let topics: Vec<&str> = data.get("topics")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter()
            .filter_map(|t| t.as_str())
            .collect())
        .unwrap_or_default();

    let visibility = if is_private { "🔒 Private" } else { "🌐 Public" };
    let fork_str = if is_fork { " (fork)" } else { "" };
    let archived_str = if is_archived { " [ARCHIVED]" } else { "" };

    let size_str = if size_kb >= 1024 {
        format!("{:.1} MB", size_kb as f64 / 1024.0)
    } else {
        format!("{size_kb} KB")
    };

    let mut out = format!("## {full_name}{fork_str}{archived_str}\n\n");
    out.push_str(&format!("{description}\n\n"));
    out.push_str(&format!("**Visibility:** {visibility}\n"));
    out.push_str(&format!("**Language:** {language} | **License:** {license}\n"));
    out.push_str(&format!("**Stars:** ⭐ {stars} | **Forks:** 🍴 {forks} | **Watchers:** 👀 {watchers}\n"));
    out.push_str(&format!("**Open Issues:** {open_issues}\n"));
    out.push_str(&format!("**Default Branch:** `{default_branch}` | **Size:** {size_str}\n"));
    out.push_str(&format!("**Created:** {} | **Last Push:** {}\n", format_time(created), format_time(pushed)));
    out.push_str(&format!("**Updated:** {}\n", format_time(updated)));

    if !topics.is_empty() {
        out.push_str(&format!("**Topics:** {}\n", topics.join(", ")));
    }

    out.push_str(&format!("**URL:** {html_url}\n"));

    // Fetch recent contributors
    if let Ok(contribs) = github_get(&format!("/repos/{owner}/{repo}/contributors?per_page=5")) {
        if let Some(contribs_arr) = contribs.as_array() {
            if !contribs_arr.is_empty() {
                out.push_str("\n### Top Contributors\n\n");
                for contrib in contribs_arr {
                    let login = contrib.get("login").and_then(|v| v.as_str()).unwrap_or("?");
                    let contributions = contrib.get("contributions").and_then(|v| v.as_u64()).unwrap_or(0);
                    out.push_str(&format!("- @{login} ({contributions} commits)\n"));
                }
            }
        }
    }

    Ok(out)
}
