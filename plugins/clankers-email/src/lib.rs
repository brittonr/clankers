//! clankers-email — Send and read emails via Fastmail JMAP.
//!
//! Uses the JMAP protocol (RFC 8620/8621) over HTTP to interact with
//! a Fastmail account. No third-party email services required.
//!
//! ## Setup
//!
//! 1. Create a Fastmail API token:
//!    Settings → Privacy & Security → API Tokens → New API Token
//!    Grant: `Mail` + `Email submission` scopes.
//!
//! 2. Set environment variables:
//!    ```
//!    export FASTMAIL_API_TOKEN=fmu1-...
//!    export CLANKERS_EMAIL_FROM=you@fastmail.com   # optional default sender
//!    ```
//!
//! ## Tools
//!
//! - **`send_email`** — Compose and send an email via JMAP `Email/set` + `EmailSubmission/set`.
//! - **`search_email`** — Search the inbox via JMAP `Email/query` + `Email/get`.
//! - **`read_email`** — Fetch a single message by ID via JMAP `Email/get`.
//! - **`list_mailboxes`** — List mailboxes (folders) in the account.

use std::collections::BTreeMap;

use clanker_plugin_sdk::http;
use clanker_plugin_sdk::prelude::*;
use clanker_plugin_sdk::serde_json;

// ═══════════════════════════════════════════════════════════════════════
//  Extism guest entrypoints
// ═══════════════════════════════════════════════════════════════════════

#[plugin_fn]
pub fn handle_tool_call(input: String) -> FnResult<String> {
    dispatch_tools(&input, &[
        ("send_email", handle_send_email),
        ("search_email", handle_search_email),
        ("read_email", handle_read_email),
        ("list_mailboxes", handle_list_mailboxes),
    ])
}

#[plugin_fn]
pub fn on_event(input: String) -> FnResult<String> {
    dispatch_events(&input, "clankers-email", &[
        ("agent_start", |_| "clankers-email: Fastmail JMAP plugin ready".to_string()),
        ("schedule_fire", handle_schedule_fire_event),
    ])
}

#[plugin_fn]
pub fn describe(Json(_): Json<()>) -> FnResult<Json<PluginMeta>> {
    Ok(Json(PluginMeta::new(
        "clankers-email",
        "0.2.0",
        &[
            ("send_email", "Send an email via Fastmail JMAP"),
            ("search_email", "Search emails via Fastmail JMAP"),
            ("read_email", "Read a single email by ID"),
            ("list_mailboxes", "List Fastmail mailboxes"),
        ],
        &[],
    )))
}

// ═══════════════════════════════════════════════════════════════════════
//  Schedule fire handler
// ═══════════════════════════════════════════════════════════════════════

/// Handle a `schedule_fire` event. The event data contains:
/// - `schedule_id`, `schedule_name`, `fire_count`
/// - `payload` — the schedule's arbitrary JSON, which we check for
///   `"action": "send_email"` and forward to `handle_send_email`.
fn handle_schedule_fire_event(data: &Value) -> String {
    let payload = match data.get("payload") {
        Some(p) => p,
        None => return "schedule_fire: no payload".to_string(),
    };

    let action = payload.get("action").and_then(|v| v.as_str()).unwrap_or("");
    if action != "send_email" {
        return format!("schedule_fire: ignoring action '{action}'");
    }

    let schedule_name = data
        .get("schedule_name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    match handle_send_email(payload) {
        Ok(result) => {
            format!("schedule '{schedule_name}' sent email: {result}")
        }
        Err(e) => {
            format!("schedule '{schedule_name}' email failed: {e}")
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  JMAP session
// ═══════════════════════════════════════════════════════════════════════

/// Minimal JMAP session info extracted from /jmap/session.
struct Session {
    api_url: String,
    account_id: String,
}

/// Fetch the JMAP session resource to discover account ID and API URL.
fn get_session(token: &str) -> Result<Session, String> {
    let mut headers = BTreeMap::new();
    headers.insert("Authorization".into(), format!("Bearer {token}"));

    let resp = http::request("GET", "https://api.fastmail.com/jmap/session", &headers, None)?;

    if !resp.is_success() {
        return Err(format!(
            "JMAP session request failed (status {}): {}",
            resp.status,
            resp.text()
        ));
    }

    let session: Value = resp.json().map_err(|e| format!("Failed to parse session: {e}"))?;

    let api_url = session
        .get("apiUrl")
        .and_then(|v| v.as_str())
        .ok_or("Session response missing 'apiUrl'")?
        .to_string();

    // primaryAccounts."urn:ietf:params:jmap:mail" → account ID
    let account_id = session
        .get("primaryAccounts")
        .and_then(|pa| pa.get("urn:ietf:params:jmap:mail"))
        .and_then(|v| v.as_str())
        .ok_or("Session response missing mail account ID")?
        .to_string();

    Ok(Session { api_url, account_id })
}

/// Make a JMAP API call (POST methodCalls to the API URL).
fn jmap_call(
    token: &str,
    api_url: &str,
    using: &[&str],
    method_calls: Value,
) -> Result<Value, String> {
    let body = serde_json::json!({
        "using": using,
        "methodCalls": method_calls,
    });

    let mut headers = BTreeMap::new();
    headers.insert("Authorization".into(), format!("Bearer {token}"));
    headers.insert("Content-Type".into(), "application/json".into());

    let resp = http::post(api_url, &headers, &body.to_string())?;

    if !resp.is_success() {
        return Err(format!(
            "JMAP API call failed (status {}): {}",
            resp.status,
            resp.text()
        ));
    }

    resp.json().map_err(|e| format!("Failed to parse JMAP response: {e}"))
}

// ═══════════════════════════════════════════════════════════════════════
//  Config helpers
// ═══════════════════════════════════════════════════════════════════════

fn require_config(key: &str) -> Result<String, String> {
    extism_pdk::config::get(key)
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Missing config: '{key}'. Set the corresponding env var."))
}

fn get_config(key: &str) -> Option<String> {
    extism_pdk::config::get(key).ok().flatten().filter(|s| !s.is_empty())
}

// ═══════════════════════════════════════════════════════════════════════
//  Recipient allowlist
// ═══════════════════════════════════════════════════════════════════════

/// Check whether an email address is permitted by the allowlist.
///
/// Each entry in `rules` is either:
/// - A full address: `alice@example.com` (exact match, case-insensitive)
/// - A domain pattern: `@example.com` (matches any address at that domain)
fn is_recipient_allowed(addr: &str, rules: &[&str]) -> bool {
    let addr = addr.trim().to_lowercase();
    for rule in rules {
        let rule = rule.trim().to_lowercase();
        if rule.starts_with('@') {
            // Domain pattern
            if addr.ends_with(&rule) {
                return true;
            }
        } else if addr == rule {
            return true;
        }
    }
    false
}

/// Validate all recipients (to + cc) against the allowlist.
/// Returns Ok(()) if all are permitted, or an error listing the rejected addresses.
fn check_allowed_recipients(to: &str, cc: Option<&str>) -> Result<(), String> {
    let allowlist = match get_config("allowed_recipients") {
        Some(list) => list,
        None => return Err(
            "No recipient allowlist configured. Set CLANKERS_EMAIL_ALLOWED_RECIPIENTS \
             (comma-separated emails or @domain patterns, e.g. \"alice@example.com, @mycompany.com\")."
                .to_string(),
        ),
    };

    let rules: Vec<&str> = allowlist.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if rules.is_empty() {
        return Err("CLANKERS_EMAIL_ALLOWED_RECIPIENTS is set but empty.".to_string());
    }

    let mut rejected = Vec::new();

    for addr in to.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if !is_recipient_allowed(addr, &rules) {
            rejected.push(addr.to_string());
        }
    }

    if let Some(cc_str) = cc {
        for addr in cc_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            if !is_recipient_allowed(addr, &rules) {
                rejected.push(addr.to_string());
            }
        }
    }

    if rejected.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Recipients not in allowlist: {}. Allowed: {}",
            rejected.join(", "),
            allowlist,
        ))
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  send_email
// ═══════════════════════════════════════════════════════════════════════

fn handle_send_email(args: &Value) -> Result<String, String> {
    let to = args.require_str("to")?;
    let subject = args.require_str("subject")?;
    let body_text = args.require_str("body")?;
    let is_html = args.get_bool_or("html", false);

    let from = match args.get_str("from") {
        Some(f) => f.to_string(),
        None => get_config("default_from")
            .ok_or("No 'from' address. Set CLANKERS_EMAIL_FROM or pass 'from' parameter.")?,
    };

    let cc = args.get_str("cc");

    // Enforce recipient allowlist before doing any network calls
    check_allowed_recipients(to, cc)?;

    let token = require_config("jmap_token")?;
    let session = get_session(&token)?;

    // Find the Drafts mailbox ID (needed for Email/set)
    let drafts_id = find_mailbox_id(&token, &session, "Drafts")?;

    // Find the identity ID (needed for EmailSubmission/set)
    let identity_id = find_identity_id(&token, &session, &from)?;

    // Build the email object
    let to_list = parse_address_list(to);
    let cc_list = cc.map(parse_address_list).unwrap_or_default();

    let (body_key, body_type) = if is_html {
        ("htmlBody", "text/html")
    } else {
        ("textBody", "text/plain")
    };

    let mut email_obj = serde_json::json!({
        "from": [{"email": from}],
        "to": to_list,
        "subject": subject,
        body_key: [{"partId": "body", "type": body_type}],
        "bodyValues": {
            "body": {
                "value": body_text,
                "isEncodingProblem": false,
                "isTruncated": false
            }
        },
        "mailboxIds": {
            drafts_id: true
        }
    });

    if !cc_list.is_empty() {
        email_obj["cc"] = serde_json::json!(cc_list);
    }

    let method_calls = serde_json::json!([
        [
            "Email/set",
            {
                "accountId": session.account_id,
                "create": {
                    "draft": email_obj
                }
            },
            "0"
        ],
        [
            "EmailSubmission/set",
            {
                "accountId": session.account_id,
                "create": {
                    "sub": {
                        "emailId": "#draft",
                        "identityId": identity_id
                    }
                },
                "onSuccessDestroyEmail": ["#sub"]
            },
            "1"
        ]
    ]);

    let response = jmap_call(
        &token,
        &session.api_url,
        &[
            "urn:ietf:params:jmap:core",
            "urn:ietf:params:jmap:mail",
            "urn:ietf:params:jmap:submission",
        ],
        method_calls,
    )?;

    // Check for errors in the response
    let method_responses = response
        .get("methodResponses")
        .and_then(|v| v.as_array())
        .ok_or("JMAP response missing 'methodResponses'")?;

    check_jmap_errors(method_responses)?;

    let mut summary = format!("Email sent to {to}");
    if cc.is_some() {
        summary.push_str(&format!(", cc: {}", cc.unwrap_or("")));
    }
    summary.push_str(&format!(" (from: {from})"));

    Ok(summary)
}

// ═══════════════════════════════════════════════════════════════════════
//  list_mailboxes
// ═══════════════════════════════════════════════════════════════════════

fn handle_list_mailboxes(args: &Value) -> Result<String, String> {
    let _ = args;

    let token = require_config("jmap_token")?;
    let session = get_session(&token)?;

    let method_calls = serde_json::json!([
        [
            "Mailbox/get",
            {
                "accountId": session.account_id,
                "properties": ["id", "name", "role", "totalEmails", "unreadEmails"]
            },
            "0"
        ]
    ]);

    let response = jmap_call(
        &token,
        &session.api_url,
        &["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
        method_calls,
    )?;

    let method_responses = response
        .get("methodResponses")
        .and_then(|v| v.as_array())
        .ok_or("JMAP response missing 'methodResponses'")?;

    let mailboxes = method_responses
        .first()
        .and_then(|mr| mr.as_array())
        .and_then(|arr| arr.get(1))
        .and_then(|result| result.get("list"))
        .and_then(|v| v.as_array())
        .ok_or("Failed to extract mailbox list from response")?;

    let mut lines = Vec::new();
    for mb in mailboxes {
        let name = mb.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        let role = mb.get("role").and_then(|v| v.as_str()).unwrap_or("-");
        let total = mb.get("totalEmails").and_then(|v| v.as_u64()).unwrap_or(0);
        let unread = mb.get("unreadEmails").and_then(|v| v.as_u64()).unwrap_or(0);
        lines.push(format!("{name} (role: {role}, total: {total}, unread: {unread})"));
    }

    Ok(lines.join("\n"))
}

// ═══════════════════════════════════════════════════════════════════════
//  search_email
// ═══════════════════════════════════════════════════════════════════════

fn handle_search_email(args: &Value) -> Result<String, String> {
    let token = require_config("jmap_token")?;
    let session = get_session(&token)?;

    // Pagination
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(20)
        .min(100) as u32;
    let offset = args
        .get("offset")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    // Resolve mailbox name → ID if provided
    let mailbox_id = match args.get("mailbox").and_then(|v| v.as_str()) {
        Some(name) => Some(find_mailbox_id(&token, &session, name)?),
        None => None,
    };

    let filter = build_search_filter(args, mailbox_id.as_deref());

    // Chain Email/query + Email/get in a single round-trip using back-references
    let method_calls = serde_json::json!([
        [
            "Email/query",
            {
                "accountId": session.account_id,
                "filter": filter,
                "sort": [{"property": "receivedAt", "isAscending": false}],
                "position": offset,
                "limit": limit,
                "calculateTotal": true
            },
            "R1"
        ],
        [
            "Email/get",
            {
                "accountId": session.account_id,
                "#ids": {
                    "resultOf": "R1",
                    "name": "Email/query",
                    "path": "/ids"
                },
                "properties": ["id", "from", "to", "subject", "receivedAt", "preview"]
            },
            "R2"
        ]
    ]);

    let response = jmap_call(
        &token,
        &session.api_url,
        &["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
        method_calls,
    )?;

    let method_responses = response
        .get("methodResponses")
        .and_then(|v| v.as_array())
        .ok_or("JMAP response missing 'methodResponses'")?;

    check_jmap_errors(method_responses)?;

    // Total from query result
    let total = method_responses
        .first()
        .and_then(|mr| mr.as_array())
        .and_then(|arr| arr.get(1))
        .and_then(|r| r.get("total"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    // Email list from get result
    let emails = method_responses
        .get(1)
        .and_then(|mr| mr.as_array())
        .and_then(|arr| arr.get(1))
        .and_then(|r| r.get("list"))
        .and_then(|v| v.as_array())
        .ok_or("Failed to extract email list from response")?;

    if emails.is_empty() {
        return Ok("No messages found.".to_string());
    }

    Ok(format_search_results(emails, total, offset, limit))
}

// ═══════════════════════════════════════════════════════════════════════
//  read_email
// ═══════════════════════════════════════════════════════════════════════

fn handle_read_email(args: &Value) -> Result<String, String> {
    let id = args.require_str("id")?;
    let token = require_config("jmap_token")?;
    let session = get_session(&token)?;

    let method_calls = serde_json::json!([
        [
            "Email/get",
            {
                "accountId": session.account_id,
                "ids": [id],
                "properties": [
                    "id", "from", "to", "cc", "subject", "receivedAt",
                    "textBody", "htmlBody", "attachments", "bodyValues"
                ],
                "fetchTextBodyValues": true,
                "fetchHTMLBodyValues": true
            },
            "0"
        ]
    ]);

    let response = jmap_call(
        &token,
        &session.api_url,
        &["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
        method_calls,
    )?;

    let method_responses = response
        .get("methodResponses")
        .and_then(|v| v.as_array())
        .ok_or("JMAP response missing 'methodResponses'")?;

    check_jmap_errors(method_responses)?;

    let result = method_responses
        .first()
        .and_then(|mr| mr.as_array())
        .and_then(|arr| arr.get(1))
        .ok_or("Missing Email/get result")?;

    // Check notFound array
    if let Some(not_found) = result.get("notFound").and_then(|v| v.as_array()) {
        if not_found.iter().any(|nf| nf.as_str() == Some(id)) {
            return Err(format!("Message not found: {id}"));
        }
    }

    let email = result
        .get("list")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .ok_or_else(|| format!("Message not found: {id}"))?;

    Ok(format_read_result(email))
}

// ═══════════════════════════════════════════════════════════════════════
//  JMAP helpers
// ═══════════════════════════════════════════════════════════════════════

/// Parse a comma-separated list of email addresses into JMAP address objects.
fn parse_address_list(input: &str) -> Vec<Value> {
    input
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|addr| serde_json::json!({"email": addr}))
        .collect()
}

/// Format JMAP address objects `[{"email":"..."}]` into a comma-separated string.
fn format_jmap_addresses(addrs: &[Value]) -> String {
    addrs
        .iter()
        .filter_map(|a| a.get("email").and_then(|v| v.as_str()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Check JMAP methodResponses for errors. Returns Ok(()) on success.
fn check_jmap_errors(method_responses: &[Value]) -> Result<(), String> {
    for mr in method_responses {
        let arr = mr.as_array().ok_or("Invalid methodResponse format")?;
        let method_name = arr.first().and_then(|v| v.as_str()).unwrap_or("");
        let result = arr.get(1).unwrap_or(&Value::Null);

        if method_name == "Email/set" {
            if let Some(not_created) = result.get("notCreated") {
                if let Some(err) = not_created.get("draft") {
                    return Err(format!(
                        "Failed to create email: {}",
                        serde_json::to_string_pretty(err).unwrap_or_default()
                    ));
                }
            }
        }

        if method_name == "EmailSubmission/set" {
            if let Some(not_created) = result.get("notCreated") {
                if let Some(err) = not_created.get("sub") {
                    return Err(format!(
                        "Email created but submission failed: {}",
                        serde_json::to_string_pretty(err).unwrap_or_default()
                    ));
                }
            }
        }

        // JMAP error responses have the method name "error"
        if method_name == "error" {
            return Err(format!(
                "JMAP error: {}",
                serde_json::to_string_pretty(result).unwrap_or_default()
            ));
        }
    }
    Ok(())
}

/// Find a mailbox by name (e.g. "Drafts") and return its ID.
///
/// Also used as `resolve_mailbox_name` for search — same lookup logic.
fn find_mailbox_id(token: &str, session: &Session, name: &str) -> Result<String, String> {
    let method_calls = serde_json::json!([
        [
            "Mailbox/get",
            {
                "accountId": session.account_id,
                "properties": ["id", "name", "role"]
            },
            "0"
        ]
    ]);

    let response = jmap_call(
        token,
        &session.api_url,
        &["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
        method_calls,
    )?;

    let mailboxes = response
        .get("methodResponses")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|mr| mr.as_array())
        .and_then(|arr| arr.get(1))
        .and_then(|result| result.get("list"))
        .and_then(|v| v.as_array())
        .ok_or("Failed to get mailbox list")?;

    // Try matching by role first (more reliable than name)
    let role_name = name.to_lowercase();
    for mb in mailboxes {
        if let Some(role) = mb.get("role").and_then(|v| v.as_str()) {
            if role.eq_ignore_ascii_case(&role_name) {
                if let Some(id) = mb.get("id").and_then(|v| v.as_str()) {
                    return Ok(id.to_string());
                }
            }
        }
    }

    // Fallback: match by name
    for mb in mailboxes {
        if let Some(mb_name) = mb.get("name").and_then(|v| v.as_str()) {
            if mb_name.eq_ignore_ascii_case(name) {
                if let Some(id) = mb.get("id").and_then(|v| v.as_str()) {
                    return Ok(id.to_string());
                }
            }
        }
    }

    Err(format!("Mailbox '{name}' not found"))
}

/// Match a `from` address against a list of JMAP identity objects.
///
/// Returns the `id` of the best matching identity, or None.
/// Matching order: exact → wildcard (`*@domain`) → first in list.
fn match_identity<'a>(identities: &'a [Value], from: &str) -> Option<&'a str> {
    let from_lower = from.to_lowercase();
    let from_domain = from_lower.rsplit_once('@').map(|(_, d)| d);

    // Exact match
    for id in identities {
        let email = id.get("email").and_then(|v| v.as_str()).unwrap_or("");
        if email.eq_ignore_ascii_case(from) {
            return id.get("id").and_then(|v| v.as_str());
        }
    }

    // Wildcard match: *@domain
    if let Some(domain) = from_domain {
        for id in identities {
            let email = id.get("email").and_then(|v| v.as_str()).unwrap_or("");
            if let Some((local, id_domain)) = email.rsplit_once('@') {
                if local == "*" && id_domain.eq_ignore_ascii_case(domain) {
                    return id.get("id").and_then(|v| v.as_str());
                }
            }
        }
    }

    // Fallback: first identity
    identities.first().and_then(|id| id.get("id")).and_then(|v| v.as_str())
}

/// Find the identity ID that matches the given `from` address.
///
/// Matching order:
/// 1. Exact match — identity email == from
/// 2. Wildcard — identity email is `*@domain` and from is `anything@domain`
/// 3. Fallback — first identity in the list
fn find_identity_id(token: &str, session: &Session, from: &str) -> Result<String, String> {
    let method_calls = serde_json::json!([
        [
            "Identity/get",
            {
                "accountId": session.account_id,
                "properties": ["id", "email", "name"]
            },
            "0"
        ]
    ]);

    let response = jmap_call(
        token,
        &session.api_url,
        &[
            "urn:ietf:params:jmap:core",
            "urn:ietf:params:jmap:submission",
        ],
        method_calls,
    )?;

    let identities = response
        .get("methodResponses")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|mr| mr.as_array())
        .and_then(|arr| arr.get(1))
        .and_then(|result| result.get("list"))
        .and_then(|v| v.as_array())
        .ok_or("Failed to get identity list")?;

    match_identity(identities, from)
        .map(|s| s.to_string())
        .ok_or_else(|| "No identities found in account".to_string())
}

// ═══════════════════════════════════════════════════════════════════════
//  Search helpers
// ═══════════════════════════════════════════════════════════════════════

/// Build a JMAP `FilterCondition` object from tool arguments.
///
/// All filter fields are optional and AND-combined by JMAP.
fn build_search_filter(args: &Value, mailbox_id: Option<&str>) -> Value {
    let mut filter = serde_json::Map::new();

    if let Some(v) = args.get("from").and_then(|v| v.as_str()) {
        filter.insert("from".into(), Value::String(v.to_string()));
    }
    if let Some(v) = args.get("to").and_then(|v| v.as_str()) {
        filter.insert("to".into(), Value::String(v.to_string()));
    }
    if let Some(v) = args.get("subject").and_then(|v| v.as_str()) {
        filter.insert("subject".into(), Value::String(v.to_string()));
    }
    if let Some(v) = args.get("query").and_then(|v| v.as_str()) {
        filter.insert("text".into(), Value::String(v.to_string()));
    }
    if let Some(v) = args.get("after").and_then(|v| v.as_str()) {
        filter.insert("after".into(), Value::String(ensure_utc_date(v)));
    }
    if let Some(v) = args.get("before").and_then(|v| v.as_str()) {
        filter.insert("before".into(), Value::String(ensure_utc_date(v)));
    }
    if let Some(id) = mailbox_id {
        filter.insert("inMailbox".into(), Value::String(id.to_string()));
    }

    Value::Object(filter)
}

/// Ensure a date string is in JMAP UTCDate format (YYYY-MM-DDTHH:MM:SSZ).
fn ensure_utc_date(date: &str) -> String {
    if date.contains('T') {
        date.to_string()
    } else {
        format!("{date}T00:00:00Z")
    }
}

/// Format search results into a readable text block.
fn format_search_results(emails: &[Value], total: u64, offset: u32, limit: u32) -> String {
    let end = offset as usize + emails.len();
    let mut lines = vec![format!(
        "Found {total} messages (showing {}-{}):",
        offset + 1,
        end,
    )];
    lines.push(String::new());

    for email in emails {
        let id = email.get("id").and_then(|v| v.as_str()).unwrap_or("?");
        let subject = email
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("(no subject)");
        let date = email
            .get("receivedAt")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let preview = email.get("preview").and_then(|v| v.as_str()).unwrap_or("");

        let from = email
            .get("from")
            .and_then(|v| v.as_array())
            .map(|a| format_jmap_addresses(a))
            .unwrap_or_default();
        let to = email
            .get("to")
            .and_then(|v| v.as_array())
            .map(|a| format_jmap_addresses(a))
            .unwrap_or_default();

        lines.push(format!("ID: {id}"));
        lines.push(format!("From: {from}"));
        if !to.is_empty() {
            lines.push(format!("To: {to}"));
        }
        lines.push(format!("Subject: {subject}"));
        lines.push(format!("Date: {date}"));
        if !preview.is_empty() {
            lines.push(format!("Preview: {preview}"));
        }
        lines.push(String::new());
    }

    if total > (offset as u64 + limit as u64) {
        lines.push(format!("Use offset={} to see more.", offset + limit));
    }

    lines.join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
//  Read helpers
// ═══════════════════════════════════════════════════════════════════════

/// Extract the body text from a JMAP email object.
///
/// Prefers text/plain body. Falls back to stripping HTML if only HTML exists.
/// Returns `(body_text, was_html_stripped)`.
fn extract_body(email: &Value, body_values: &Value) -> (String, bool) {
    // Try textBody first
    if let Some(text) = body_part_value(email, "textBody", body_values) {
        if !text.is_empty() {
            return (text, false);
        }
    }

    // Fallback: htmlBody → strip tags
    if let Some(html) = body_part_value(email, "htmlBody", body_values) {
        if !html.is_empty() {
            return (html_to_text(&html), true);
        }
    }

    ("(no body)".to_string(), false)
}

/// Get the text value of the first body part in `key` (textBody or htmlBody).
fn body_part_value(email: &Value, key: &str, body_values: &Value) -> Option<String> {
    let part_id = email
        .get(key)
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|p| p.get("partId"))
        .and_then(|v| v.as_str())?;

    body_values
        .get(part_id)
        .and_then(|bv| bv.get("value"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Extract attachment metadata from a JMAP email object.
///
/// Returns `(filename, content_type, size)` tuples.
fn extract_attachments(email: &Value) -> Vec<(String, String, u64)> {
    let mut result = Vec::new();
    if let Some(attachments) = email.get("attachments").and_then(|v| v.as_array()) {
        for att in attachments {
            let name = att
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unnamed")
                .to_string();
            let content_type = att
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("application/octet-stream")
                .to_string();
            let size = att.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
            result.push((name, content_type, size));
        }
    }
    result
}

/// Format a full email message for read_email output.
fn format_read_result(email: &Value) -> String {
    let id = email.get("id").and_then(|v| v.as_str()).unwrap_or("?");
    let subject = email
        .get("subject")
        .and_then(|v| v.as_str())
        .unwrap_or("(no subject)");
    let date = email
        .get("receivedAt")
        .and_then(|v| v.as_str())
        .unwrap_or("?");

    let from = email
        .get("from")
        .and_then(|v| v.as_array())
        .map(|a| format_jmap_addresses(a))
        .unwrap_or_default();
    let to = email
        .get("to")
        .and_then(|v| v.as_array())
        .map(|a| format_jmap_addresses(a))
        .unwrap_or_default();
    let cc = email
        .get("cc")
        .and_then(|v| v.as_array())
        .map(|a| format_jmap_addresses(a))
        .unwrap_or_default();

    let body_values = email.get("bodyValues").unwrap_or(&Value::Null);
    let (body, html_stripped) = extract_body(email, body_values);
    let attachments = extract_attachments(email);

    let mut lines = Vec::new();
    lines.push(format!("ID: {id}"));
    lines.push(format!("From: {from}"));
    if !to.is_empty() {
        lines.push(format!("To: {to}"));
    }
    if !cc.is_empty() {
        lines.push(format!("CC: {cc}"));
    }
    lines.push(format!("Subject: {subject}"));
    lines.push(format!("Date: {date}"));
    if html_stripped {
        lines.push("(html_stripped: true)".to_string());
    }
    lines.push(String::new());
    lines.push(body);

    if !attachments.is_empty() {
        lines.push(String::new());
        lines.push("Attachments:".to_string());
        for (name, content_type, size) in &attachments {
            lines.push(format!("  - {name} ({content_type}, {size} bytes)"));
        }
    }

    lines.join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
//  HTML → text
// ═══════════════════════════════════════════════════════════════════════

/// Strip HTML tags, decode common entities, collapse whitespace.
///
/// Not a full HTML parser — good enough for extracting readable text
/// from email bodies without pulling in heavy dependencies.
fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut tag_buf = String::new();

    let chars: Vec<char> = html.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if ch == '<' {
            in_tag = true;
            tag_buf.clear();
            i += 1;
            continue;
        }

        if in_tag {
            if ch == '>' {
                in_tag = false;
                let tag_lower = tag_buf.trim().to_lowercase();
                // Block-level elements → newline
                if tag_lower.starts_with("br")
                    || tag_lower.starts_with("p")
                    || tag_lower.starts_with("/p")
                    || tag_lower.starts_with("div")
                    || tag_lower.starts_with("/div")
                    || tag_lower.starts_with("tr")
                    || tag_lower.starts_with("/tr")
                    || tag_lower.starts_with("li")
                    || tag_lower.starts_with("h1")
                    || tag_lower.starts_with("h2")
                    || tag_lower.starts_with("h3")
                    || tag_lower.starts_with("/h1")
                    || tag_lower.starts_with("/h2")
                    || tag_lower.starts_with("/h3")
                {
                    out.push('\n');
                }
                if tag_lower.starts_with("li") {
                    out.push_str("- ");
                }
            } else {
                tag_buf.push(ch);
            }
            i += 1;
            continue;
        }

        // Entity decoding
        if ch == '&' {
            if let Some(decoded) = decode_entity(&chars, i) {
                out.push_str(&decoded.0);
                i = decoded.1;
                continue;
            }
        }

        out.push(ch);
        i += 1;
    }

    collapse_whitespace(&out)
}

/// Try to decode an HTML entity starting at position `start` (the `&`).
/// Returns `(decoded_string, next_index)` or `None` if not a valid entity.
fn decode_entity(chars: &[char], start: usize) -> Option<(String, usize)> {
    // Find the semicolon (max 10 chars to avoid scanning forever)
    let max = (start + 12).min(chars.len());
    let semi_pos = (start + 1..max).find(|&j| chars[j] == ';')?;

    let entity: String = chars[start + 1..semi_pos].iter().collect();
    let replacement = match entity.as_str() {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => " ",
        _ if entity.starts_with('#') => {
            let num_str = &entity[1..];
            let code_point = if let Some(hex) = num_str.strip_prefix('x').or(num_str.strip_prefix('X')) {
                u32::from_str_radix(hex, 16).ok()
            } else {
                num_str.parse::<u32>().ok()
            };
            if let Some(cp) = code_point {
                if let Some(c) = char::from_u32(cp) {
                    // Return directly to avoid lifetime issues with temp string
                    return Some((c.to_string(), semi_pos + 1));
                }
            }
            return None;
        }
        _ => return None,
    };

    Some((replacement.to_string(), semi_pos + 1))
}

/// Collapse runs of whitespace into single spaces, trim lines.
fn collapse_whitespace(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();

    for line in text.split('\n') {
        // Collapse horizontal whitespace within the line
        let mut collapsed = String::new();
        let mut prev_space = true; // trim leading
        for ch in line.chars() {
            if ch.is_whitespace() {
                if !prev_space {
                    collapsed.push(' ');
                    prev_space = true;
                }
            } else {
                collapsed.push(ch);
                prev_space = false;
            }
        }
        lines.push(collapsed.trim_end().to_string());
    }

    // Remove leading/trailing blank lines, collapse runs of blank lines to one
    let mut result = Vec::new();
    let mut prev_blank = true;
    for line in &lines {
        if line.is_empty() {
            if !prev_blank && !result.is_empty() {
                result.push(String::new());
                prev_blank = true;
            }
        } else {
            result.push(line.clone());
            prev_blank = false;
        }
    }

    // Trim trailing blank lines
    while result.last().map_or(false, |l| l.is_empty()) {
        result.pop();
    }

    result.join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
//  Tests — pure logic only, no WASM runtime needed
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Address parsing ─────────────────────────────────────────────

    #[test]
    fn parse_single_address() {
        let list = parse_address_list("alice@example.com");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["email"], "alice@example.com");
    }

    #[test]
    fn parse_multiple_addresses() {
        let list = parse_address_list("alice@example.com, bob@example.com, carol@example.com");
        assert_eq!(list.len(), 3);
        assert_eq!(list[0]["email"], "alice@example.com");
        assert_eq!(list[1]["email"], "bob@example.com");
        assert_eq!(list[2]["email"], "carol@example.com");
    }

    #[test]
    fn parse_addresses_trims_whitespace() {
        let list = parse_address_list("  alice@example.com ,  bob@example.com  ");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["email"], "alice@example.com");
        assert_eq!(list[1]["email"], "bob@example.com");
    }

    #[test]
    fn parse_addresses_skips_empty() {
        let list = parse_address_list("alice@example.com,,, bob@example.com,");
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn parse_empty_string() {
        let list = parse_address_list("");
        assert!(list.is_empty());
    }

    // ── Address formatting ──────────────────────────────────────────

    #[test]
    fn format_addresses_single() {
        let addrs = vec![serde_json::json!({"email": "a@b.com"})];
        assert_eq!(format_jmap_addresses(&addrs), "a@b.com");
    }

    #[test]
    fn format_addresses_multiple() {
        let addrs = vec![
            serde_json::json!({"email": "a@b.com"}),
            serde_json::json!({"email": "c@d.com"}),
        ];
        assert_eq!(format_jmap_addresses(&addrs), "a@b.com, c@d.com");
    }

    #[test]
    fn format_addresses_empty() {
        let addrs: Vec<Value> = vec![];
        assert_eq!(format_jmap_addresses(&addrs), "");
    }

    // ── JMAP error checking ─────────────────────────────────────────

    #[test]
    fn check_jmap_errors_success_response() {
        let responses = vec![
            serde_json::json!(["Email/set", {"created": {"draft": {"id": "abc"}}}, "0"]),
            serde_json::json!(["EmailSubmission/set", {"created": {"sub": {"id": "def"}}}, "1"]),
        ];
        assert!(check_jmap_errors(&responses).is_ok());
    }

    #[test]
    fn check_jmap_errors_email_not_created() {
        let responses = vec![
            serde_json::json!(["Email/set", {
                "notCreated": {
                    "draft": {"type": "invalidProperties", "description": "bad subject"}
                }
            }, "0"]),
        ];
        let err = check_jmap_errors(&responses).unwrap_err();
        assert!(err.contains("Failed to create email"), "got: {err}");
    }

    #[test]
    fn check_jmap_errors_submission_not_created() {
        let responses = vec![
            serde_json::json!(["Email/set", {"created": {"draft": {"id": "abc"}}}, "0"]),
            serde_json::json!(["EmailSubmission/set", {
                "notCreated": {
                    "sub": {"type": "forbidden", "description": "not allowed"}
                }
            }, "1"]),
        ];
        let err = check_jmap_errors(&responses).unwrap_err();
        assert!(err.contains("submission failed"), "got: {err}");
    }

    #[test]
    fn check_jmap_errors_method_error() {
        let responses = vec![
            serde_json::json!(["error", {"type": "unknownMethod"}, "0"]),
        ];
        let err = check_jmap_errors(&responses).unwrap_err();
        assert!(err.contains("JMAP error"), "got: {err}");
    }

    #[test]
    fn check_jmap_errors_empty_responses() {
        let responses: Vec<Value> = vec![];
        assert!(check_jmap_errors(&responses).is_ok());
    }

    // ── Recipient allowlist ────────────────────────────────────────

    #[test]
    fn allowlist_exact_match() {
        let rules = vec!["alice@example.com"];
        assert!(is_recipient_allowed("alice@example.com", &rules));
    }

    #[test]
    fn allowlist_exact_match_case_insensitive() {
        let rules = vec!["Alice@Example.COM"];
        assert!(is_recipient_allowed("alice@example.com", &rules));
        assert!(is_recipient_allowed("ALICE@EXAMPLE.COM", &rules));
    }

    #[test]
    fn allowlist_exact_no_match() {
        let rules = vec!["alice@example.com"];
        assert!(!is_recipient_allowed("bob@example.com", &rules));
    }

    #[test]
    fn allowlist_domain_match() {
        let rules = vec!["@example.com"];
        assert!(is_recipient_allowed("alice@example.com", &rules));
        assert!(is_recipient_allowed("bob@example.com", &rules));
        assert!(is_recipient_allowed("anyone@example.com", &rules));
    }

    #[test]
    fn allowlist_domain_no_match() {
        let rules = vec!["@example.com"];
        assert!(!is_recipient_allowed("alice@other.com", &rules));
    }

    #[test]
    fn allowlist_domain_case_insensitive() {
        let rules = vec!["@Example.COM"];
        assert!(is_recipient_allowed("alice@example.com", &rules));
    }

    #[test]
    fn allowlist_mixed_rules() {
        let rules = vec!["specific@other.com", "@example.com"];
        assert!(is_recipient_allowed("anyone@example.com", &rules));
        assert!(is_recipient_allowed("specific@other.com", &rules));
        assert!(!is_recipient_allowed("random@other.com", &rules));
    }

    #[test]
    fn allowlist_empty_rules_rejects_all() {
        let rules: Vec<&str> = vec![];
        assert!(!is_recipient_allowed("alice@example.com", &rules));
    }

    #[test]
    fn allowlist_trims_whitespace() {
        let rules = vec!["  alice@example.com  ", "  @other.com  "];
        assert!(is_recipient_allowed("alice@example.com", &rules));
        assert!(is_recipient_allowed("bob@other.com", &rules));
    }

    // ── Identity matching ───────────────────────────────────────────

    fn make_identities() -> Vec<Value> {
        vec![
            serde_json::json!({"id": "100", "email": "alice@example.com"}),
            serde_json::json!({"id": "200", "email": "*@example.com"}),
            serde_json::json!({"id": "300", "email": "bob@other.com"}),
        ]
    }

    #[test]
    fn identity_exact_match() {
        let ids = make_identities();
        assert_eq!(match_identity(&ids, "alice@example.com"), Some("100"));
    }

    #[test]
    fn identity_exact_match_case_insensitive() {
        let ids = make_identities();
        assert_eq!(match_identity(&ids, "Alice@Example.COM"), Some("100"));
    }

    #[test]
    fn identity_wildcard_match() {
        let ids = make_identities();
        // "noreply@example.com" doesn't match alice exactly, falls through to *@example.com
        assert_eq!(match_identity(&ids, "noreply@example.com"), Some("200"));
    }

    #[test]
    fn identity_wildcard_case_insensitive() {
        let ids = make_identities();
        assert_eq!(match_identity(&ids, "Anything@EXAMPLE.COM"), Some("200"));
    }

    #[test]
    fn identity_no_match_falls_back_to_first() {
        let ids = make_identities();
        // "someone@unknown.org" matches nothing — falls back to first
        assert_eq!(match_identity(&ids, "someone@unknown.org"), Some("100"));
    }

    #[test]
    fn identity_exact_beats_wildcard() {
        let ids = make_identities();
        // alice@example.com matches exactly, even though *@example.com also covers it
        assert_eq!(match_identity(&ids, "alice@example.com"), Some("100"));
    }

    #[test]
    fn identity_empty_list() {
        let ids: Vec<Value> = vec![];
        assert_eq!(match_identity(&ids, "anyone@example.com"), None);
    }

    #[test]
    fn identity_wildcard_different_domain_no_match() {
        let ids = make_identities();
        // *@example.com should NOT match someone@other.com
        // bob@other.com is exact, so it matches
        assert_eq!(match_identity(&ids, "bob@other.com"), Some("300"));
        // unknown@other.com has no exact or wildcard, falls back to first
        assert_eq!(match_identity(&ids, "unknown@other.com"), Some("100"));
    }

    // ── html_to_text ────────────────────────────────────────────────

    #[test]
    fn html_to_text_strips_basic_tags() {
        assert_eq!(html_to_text("<b>bold</b> and <i>italic</i>"), "bold and italic");
    }

    #[test]
    fn html_to_text_handles_nested_tags() {
        assert_eq!(
            html_to_text("<div><p><b>nested</b> content</p></div>"),
            "nested content"
        );
    }

    #[test]
    fn html_to_text_converts_br_to_newline() {
        assert_eq!(html_to_text("line one<br>line two"), "line one\nline two");
    }

    #[test]
    fn html_to_text_converts_p_to_newline() {
        assert_eq!(
            html_to_text("<p>para one</p><p>para two</p>"),
            "para one\n\npara two"
        );
    }

    #[test]
    fn html_to_text_converts_li_to_dash() {
        let result = html_to_text("<ul><li>first</li><li>second</li></ul>");
        assert!(result.contains("- first"), "got: {result}");
        assert!(result.contains("- second"), "got: {result}");
    }

    #[test]
    fn html_to_text_decodes_named_entities() {
        assert_eq!(html_to_text("&amp; &lt; &gt; &quot;"), "& < > \"");
    }

    #[test]
    fn html_to_text_decodes_numeric_entities() {
        // &#65; = 'A', &#x41; = 'A'
        assert_eq!(html_to_text("&#65; &#x41;"), "A A");
    }

    #[test]
    fn html_to_text_decodes_nbsp() {
        assert_eq!(html_to_text("hello&nbsp;world"), "hello world");
    }

    #[test]
    fn html_to_text_empty_input() {
        assert_eq!(html_to_text(""), "");
    }

    #[test]
    fn html_to_text_collapses_whitespace() {
        assert_eq!(html_to_text("  lots   of    spaces  "), "lots of spaces");
    }

    #[test]
    fn html_to_text_preserves_plain_text() {
        assert_eq!(html_to_text("no tags here"), "no tags here");
    }

    // ── ensure_utc_date ─────────────────────────────────────────────

    #[test]
    fn utc_date_appends_time() {
        assert_eq!(ensure_utc_date("2025-01-15"), "2025-01-15T00:00:00Z");
    }

    #[test]
    fn utc_date_passthrough_full() {
        assert_eq!(
            ensure_utc_date("2025-01-15T10:30:00Z"),
            "2025-01-15T10:30:00Z"
        );
    }

    // ── build_search_filter ─────────────────────────────────────────

    #[test]
    fn filter_empty_args() {
        let args = serde_json::json!({});
        let filter = build_search_filter(&args, None);
        assert_eq!(filter, serde_json::json!({}));
    }

    #[test]
    fn filter_from_only() {
        let args = serde_json::json!({"from": "alice@example.com"});
        let filter = build_search_filter(&args, None);
        assert_eq!(filter["from"], "alice@example.com");
    }

    #[test]
    fn filter_multiple_fields() {
        let args = serde_json::json!({
            "from": "alice@example.com",
            "subject": "report",
            "after": "2025-01-01"
        });
        let filter = build_search_filter(&args, None);
        assert_eq!(filter["from"], "alice@example.com");
        assert_eq!(filter["subject"], "report");
        assert_eq!(filter["after"], "2025-01-01T00:00:00Z");
    }

    #[test]
    fn filter_query_maps_to_text() {
        let args = serde_json::json!({"query": "quarterly"});
        let filter = build_search_filter(&args, None);
        assert_eq!(filter["text"], "quarterly");
    }

    #[test]
    fn filter_with_mailbox_id() {
        let args = serde_json::json!({});
        let filter = build_search_filter(&args, Some("mbox-123"));
        assert_eq!(filter["inMailbox"], "mbox-123");
    }

    // ── format_search_results ───────────────────────────────────────

    fn make_search_email(id: &str, from: &str, subject: &str) -> Value {
        serde_json::json!({
            "id": id,
            "from": [{"email": from}],
            "to": [{"email": "me@example.com"}],
            "subject": subject,
            "receivedAt": "2025-03-15T10:00:00Z",
            "preview": "Preview text here..."
        })
    }

    #[test]
    fn format_search_results_basic() {
        let emails = vec![make_search_email("abc", "alice@co.com", "Hello")];
        let result = format_search_results(&emails, 1, 0, 20);
        assert!(result.contains("Found 1 messages"), "got: {result}");
        assert!(result.contains("ID: abc"), "got: {result}");
        assert!(result.contains("From: alice@co.com"), "got: {result}");
        assert!(result.contains("Subject: Hello"), "got: {result}");
    }

    #[test]
    fn format_search_results_pagination_hint() {
        let emails = vec![make_search_email("a", "x@x.com", "S")];
        let result = format_search_results(&emails, 50, 0, 20);
        assert!(result.contains("Use offset=20 to see more"), "got: {result}");
    }

    #[test]
    fn format_search_results_no_pagination_hint_at_end() {
        let emails = vec![make_search_email("a", "x@x.com", "S")];
        let result = format_search_results(&emails, 1, 0, 20);
        assert!(!result.contains("offset="), "got: {result}");
    }

    // ── extract_body ────────────────────────────────────────────────

    #[test]
    fn extract_body_plain_text() {
        let email = serde_json::json!({
            "textBody": [{"partId": "1"}],
            "htmlBody": [{"partId": "2"}]
        });
        let body_values = serde_json::json!({
            "1": {"value": "Plain text content"},
            "2": {"value": "<p>HTML content</p>"}
        });
        let (body, stripped) = extract_body(&email, &body_values);
        assert_eq!(body, "Plain text content");
        assert!(!stripped);
    }

    #[test]
    fn extract_body_html_fallback() {
        let email = serde_json::json!({
            "textBody": [{"partId": "1"}],
            "htmlBody": [{"partId": "2"}]
        });
        let body_values = serde_json::json!({
            "1": {"value": ""},
            "2": {"value": "<p>HTML <b>content</b></p>"}
        });
        let (body, stripped) = extract_body(&email, &body_values);
        assert!(body.contains("HTML content"), "got: {body}");
        assert!(stripped);
    }

    #[test]
    fn extract_body_no_body() {
        let email = serde_json::json!({});
        let body_values = serde_json::json!({});
        let (body, stripped) = extract_body(&email, &body_values);
        assert_eq!(body, "(no body)");
        assert!(!stripped);
    }

    #[test]
    fn extract_body_html_only_no_text_part() {
        let email = serde_json::json!({
            "htmlBody": [{"partId": "h1"}]
        });
        let body_values = serde_json::json!({
            "h1": {"value": "<div>Hello world</div>"}
        });
        let (body, stripped) = extract_body(&email, &body_values);
        assert!(body.contains("Hello world"), "got: {body}");
        assert!(stripped);
    }

    // ── extract_attachments ─────────────────────────────────────────

    #[test]
    fn extract_attachments_present() {
        let email = serde_json::json!({
            "attachments": [
                {"name": "doc.pdf", "type": "application/pdf", "size": 1024},
                {"name": "pic.png", "type": "image/png", "size": 2048}
            ]
        });
        let atts = extract_attachments(&email);
        assert_eq!(atts.len(), 2);
        assert_eq!(atts[0], ("doc.pdf".into(), "application/pdf".into(), 1024));
        assert_eq!(atts[1], ("pic.png".into(), "image/png".into(), 2048));
    }

    #[test]
    fn extract_attachments_empty() {
        let email = serde_json::json!({"attachments": []});
        assert!(extract_attachments(&email).is_empty());
    }

    #[test]
    fn extract_attachments_missing_field() {
        let email = serde_json::json!({});
        assert!(extract_attachments(&email).is_empty());
    }

    // ── format_read_result ──────────────────────────────────────────

    #[test]
    fn format_read_result_full() {
        let email = serde_json::json!({
            "id": "msg-1",
            "from": [{"email": "sender@co.com"}],
            "to": [{"email": "me@co.com"}],
            "cc": [{"email": "other@co.com"}],
            "subject": "Test Subject",
            "receivedAt": "2025-03-15T10:00:00Z",
            "textBody": [{"partId": "t1"}],
            "bodyValues": {
                "t1": {"value": "Hello, this is the body."}
            },
            "attachments": [
                {"name": "file.txt", "type": "text/plain", "size": 42}
            ]
        });
        let result = format_read_result(&email);
        assert!(result.contains("ID: msg-1"), "got: {result}");
        assert!(result.contains("From: sender@co.com"), "got: {result}");
        assert!(result.contains("To: me@co.com"), "got: {result}");
        assert!(result.contains("CC: other@co.com"), "got: {result}");
        assert!(result.contains("Subject: Test Subject"), "got: {result}");
        assert!(result.contains("Hello, this is the body."), "got: {result}");
        assert!(result.contains("file.txt (text/plain, 42 bytes)"), "got: {result}");
    }

    #[test]
    fn format_read_result_html_stripped_flag() {
        let email = serde_json::json!({
            "id": "msg-2",
            "from": [{"email": "x@x.com"}],
            "subject": "HTML only",
            "receivedAt": "2025-03-15T10:00:00Z",
            "textBody": [{"partId": "t1"}],
            "htmlBody": [{"partId": "h1"}],
            "bodyValues": {
                "t1": {"value": ""},
                "h1": {"value": "<p>HTML body</p>"}
            }
        });
        let result = format_read_result(&email);
        assert!(result.contains("(html_stripped: true)"), "got: {result}");
        assert!(result.contains("HTML body"), "got: {result}");
    }

    #[test]
    fn format_read_result_no_attachments() {
        let email = serde_json::json!({
            "id": "msg-3",
            "from": [{"email": "x@x.com"}],
            "subject": "No attachments",
            "receivedAt": "2025-03-15T10:00:00Z",
            "textBody": [{"partId": "t1"}],
            "bodyValues": {
                "t1": {"value": "Just text."}
            }
        });
        let result = format_read_result(&email);
        assert!(!result.contains("Attachments:"), "got: {result}");
    }
}
