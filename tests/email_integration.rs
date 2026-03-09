//! Integration tests for the clankers-email plugin against live Fastmail JMAP.
//!
//! These tests require real credentials:
//!   FASTMAIL_API_TOKEN                — Fastmail API token
//!   CLANKERS_EMAIL_FROM               — sender email address
//!   CLANKERS_EMAIL_ALLOWED_RECIPIENTS — recipient allowlist
//!
//! Skipped automatically when any of these env vars are absent.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use clankers::plugin::PluginManager;

/// Build a PluginManager with the email plugin loaded and configured.
/// Returns None if any required env var is missing.
fn load_email_plugin() -> Option<Arc<Mutex<PluginManager>>> {
    for var in [
        "FASTMAIL_API_TOKEN",
        "CLANKERS_EMAIL_FROM",
        "CLANKERS_EMAIL_ALLOWED_RECIPIENTS",
    ] {
        if std::env::var(var).is_err() {
            return None;
        }
    }

    let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let mut mgr = PluginManager::new(plugins_dir, None);
    mgr.discover();
    mgr.load_wasm("clankers-email").expect("Failed to load email plugin WASM");
    Some(Arc::new(Mutex::new(mgr)))
}

fn call(mgr: &Arc<Mutex<PluginManager>>, input: &str) -> serde_json::Value {
    let m = mgr.lock().unwrap();
    let raw = m.call_plugin("clankers-email", "handle_tool_call", input).expect("plugin call failed");
    serde_json::from_str(&raw).expect("invalid JSON response")
}

// ── list_mailboxes ──────────────────────────────────────────────────

#[test]
fn list_mailboxes_returns_folders() {
    let mgr = match load_email_plugin() {
        Some(m) => m,
        None => {
            eprintln!("SKIP: email env vars not set");
            return;
        }
    };

    let input = r#"{"tool":"list_mailboxes","args":{}}"#;
    let resp = call(&mgr, input);

    assert_eq!(resp["status"], "ok", "list_mailboxes failed: {:?}", resp);

    let result = resp["result"].as_str().expect("result should be a string");
    assert!(!result.is_empty(), "mailbox list should not be empty");

    // Every Fastmail account has these standard mailboxes
    let lower = result.to_lowercase();
    assert!(lower.contains("inbox"), "should contain Inbox, got:\n{result}");
    assert!(lower.contains("drafts"), "should contain Drafts, got:\n{result}");
    assert!(lower.contains("sent"), "should contain Sent, got:\n{result}");
    assert!(lower.contains("trash"), "should contain Trash, got:\n{result}");
}

// ── send_email ──────────────────────────────────────────────────────

#[test]
fn send_email_to_self() {
    let mgr = match load_email_plugin() {
        Some(m) => m,
        None => {
            eprintln!("SKIP: email env vars not set");
            return;
        }
    };

    let from = std::env::var("CLANKERS_EMAIL_FROM").expect("CLANKERS_EMAIL_FROM must be set");

    let input = serde_json::json!({
        "tool": "send_email",
        "args": {
            "to": from,
            "from": from,
            "subject": "clankers integration test",
            "body": "This is an automated test from clankers-email plugin integration tests. Safe to delete."
        }
    });

    let resp = call(&mgr, &input.to_string());

    assert_eq!(resp["status"], "ok", "send_email failed: {:?}", resp);

    let result = resp["result"].as_str().expect("result should be a string");
    assert!(result.contains("Email sent"), "expected success message, got: {result}");
    assert!(result.contains(&from), "should mention recipient, got: {result}");
}

// ── send_email with cc ──────────────────────────────────────────────

#[test]
fn send_email_with_cc() {
    let mgr = match load_email_plugin() {
        Some(m) => m,
        None => {
            eprintln!("SKIP: email env vars not set");
            return;
        }
    };

    let from = std::env::var("CLANKERS_EMAIL_FROM").expect("CLANKERS_EMAIL_FROM must be set");

    let input = serde_json::json!({
        "tool": "send_email",
        "args": {
            "to": from,
            "from": from,
            "cc": from,
            "subject": "clankers integration test (cc)",
            "body": "Testing CC field. Safe to delete."
        }
    });

    let resp = call(&mgr, &input.to_string());

    assert_eq!(resp["status"], "ok", "send_email with cc failed: {:?}", resp);
}

// ── send_email html ─────────────────────────────────────────────────

#[test]
fn send_email_html() {
    let mgr = match load_email_plugin() {
        Some(m) => m,
        None => {
            eprintln!("SKIP: email env vars not set");
            return;
        }
    };

    let from = std::env::var("CLANKERS_EMAIL_FROM").expect("CLANKERS_EMAIL_FROM must be set");

    let input = serde_json::json!({
        "tool": "send_email",
        "args": {
            "to": from,
            "from": from,
            "subject": "clankers integration test (html)",
            "body": "<h1>Hello</h1><p>This is an <b>HTML</b> test email from clankers. Safe to delete.</p>",
            "html": true
        }
    });

    let resp = call(&mgr, &input.to_string());

    assert_eq!(resp["status"], "ok", "send_email html failed: {:?}", resp);
}

// ── error cases with real plugin ────────────────────────────────────

#[test]
fn send_email_missing_required_field() {
    let mgr = match load_email_plugin() {
        Some(m) => m,
        None => {
            eprintln!("SKIP: email env vars not set");
            return;
        }
    };

    // Missing "subject"
    let input = r#"{"tool":"send_email","args":{"to":"x@x.com","body":"hi","from":"x@x.com"}}"#;
    let resp = call(&mgr, input);

    assert_ne!(resp["status"], "ok", "should fail without subject: {:?}", resp);
    let result = resp["result"].as_str().unwrap_or("");
    assert!(result.contains("subject"), "error should mention subject, got: {result}");
}

#[test]
fn send_email_disallowed_recipient_blocked() {
    let mgr = match load_email_plugin() {
        Some(m) => m,
        None => {
            eprintln!("SKIP: email env vars not set");
            return;
        }
    };

    let from = std::env::var("CLANKERS_EMAIL_FROM").expect("CLANKERS_EMAIL_FROM must be set");

    let input = serde_json::json!({
        "tool": "send_email",
        "args": {
            "to": "nobody@example.invalid",
            "from": from,
            "subject": "should be blocked",
            "body": "This should never be sent."
        }
    });

    let resp = call(&mgr, &input.to_string());

    assert_ne!(resp["status"], "ok", "should reject recipient not in allowlist: {:?}", resp);
    let result = resp["result"].as_str().unwrap_or("");
    assert!(result.contains("allowlist"), "error should mention allowlist, got: {result}");
    assert!(result.contains("nobody@example.invalid"), "error should name the rejected address, got: {result}");
}

#[test]
fn send_email_disallowed_cc_blocked() {
    let mgr = match load_email_plugin() {
        Some(m) => m,
        None => {
            eprintln!("SKIP: email env vars not set");
            return;
        }
    };

    let from = std::env::var("CLANKERS_EMAIL_FROM").expect("CLANKERS_EMAIL_FROM must be set");

    let input = serde_json::json!({
        "tool": "send_email",
        "args": {
            "to": from,
            "cc": "sneaky@evil.com",
            "from": from,
            "subject": "cc should be blocked",
            "body": "The CC should be rejected."
        }
    });

    let resp = call(&mgr, &input.to_string());

    assert_ne!(resp["status"], "ok", "should reject CC not in allowlist: {:?}", resp);
    let result = resp["result"].as_str().unwrap_or("");
    assert!(result.contains("sneaky@evil.com"), "error should name the rejected CC, got: {result}");
}
