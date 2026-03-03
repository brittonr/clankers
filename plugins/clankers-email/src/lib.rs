//! clankers-email — Send emails via Fastmail JMAP.
//!
//! Uses the JMAP protocol (RFC 8620/8621) over HTTP to send email directly
//! through a Fastmail account. No third-party email services required.
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
//! - **`list_mailboxes`** — List mailboxes (folders) in the account.

use std::collections::BTreeMap;

use clankers_plugin_sdk::http;
use clankers_plugin_sdk::prelude::*;
use clankers_plugin_sdk::serde_json;

// ═══════════════════════════════════════════════════════════════════════
//  Extism guest entrypoints
// ═══════════════════════════════════════════════════════════════════════

#[plugin_fn]
pub fn handle_tool_call(input: String) -> FnResult<String> {
    dispatch_tools(&input, &[
        ("send_email", handle_send_email),
        ("list_mailboxes", handle_list_mailboxes),
    ])
}

#[plugin_fn]
pub fn on_event(input: String) -> FnResult<String> {
    dispatch_events(&input, "clankers-email", &[
        ("agent_start", |_| "clankers-email: Fastmail JMAP plugin ready".to_string()),
    ])
}

#[plugin_fn]
pub fn describe(Json(_): Json<()>) -> FnResult<Json<PluginMeta>> {
    Ok(Json(PluginMeta::new(
        "clankers-email",
        "0.1.0",
        &[
            ("send_email", "Send an email via Fastmail JMAP"),
            ("list_mailboxes", "List Fastmail mailboxes"),
        ],
        &[],
    )))
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

    let token = require_config("jmap_token")?;
    let session = get_session(&token)?;

    // Find the Drafts mailbox ID (needed for Email/set)
    let drafts_id = find_mailbox_id(&token, &session, "Drafts")?;

    // Find the identity ID (needed for EmailSubmission/set)
    let identity_id = find_identity_id(&token, &session)?;

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

/// Find the primary identity ID for the account.
fn find_identity_id(token: &str, session: &Session) -> Result<String, String> {
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

    // Return the first identity
    identities
        .first()
        .and_then(|id| id.get("id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "No identities found in account".to_string())
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
}
