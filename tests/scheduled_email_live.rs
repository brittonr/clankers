//! Live integration test: scheduler fires → plugin dispatches email via JMAP.
//!
//! Tests the full path:
//!   ScheduleEngine tick → ScheduleEvent → DaemonEvent::ScheduleFire
//!     → plugin dispatch → clankers-email WASM → Fastmail JMAP → email arrives
//!
//! Uses a native reqwest JMAP client only for *verification* (searching
//! for the sent email). The actual sending goes through the WASM plugin.
//!
//! Requires sops-encrypted secrets in onix-core. Skips gracefully if
//! secrets are unavailable.
//!
//! Run with:
//!   cargo nextest run --test scheduled_email_live
//!   cargo test --test scheduled_email_live -- --nocapture

use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use clanker_scheduler::{Schedule, ScheduleEngine};
use clankers_protocol::DaemonEvent;
use serde_json::json;

// ═══════════════════════════════════════════════════════════════════════
//  Secret loading via sops
// ═══════════════════════════════════════════════════════════════════════

const SOPS_BASE: &str = "/home/brittonr/git/onix-core/vars/shared/clankers-daemon-clankers";

fn sops_decrypt(secret_path: &str) -> Option<String> {
    let full_path = format!("{SOPS_BASE}/{secret_path}/secret");
    let output = Command::new("nix")
        .args(["run", "nixpkgs#sops", "--", "-d", &full_path])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let val = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if val.is_empty() { None } else { Some(val) }
}

struct Secrets {
    api_token: String,
    email_from: String,
}

fn load_secrets() -> Option<Secrets> {
    Some(Secrets {
        api_token: sops_decrypt("fastmail-api-token")?,
        email_from: sops_decrypt("email-from")?,
    })
}

const TEST_RECIPIENT: &str = "clanker@robitzs.ch";

fn test_subject() -> String {
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    format!("[scheduled-email-test] {ts}")
}

// ═══════════════════════════════════════════════════════════════════════
//  JMAP verification client (native reqwest, test-only)
// ═══════════════════════════════════════════════════════════════════════

struct JmapVerifier {
    token: String,
    http: reqwest::Client,
}

impl JmapVerifier {
    fn new(token: String) -> Self {
        Self { token, http: reqwest::Client::new() }
    }

    async fn session(&self) -> Result<(String, String), String> {
        let resp = self.http
            .get("https://api.fastmail.com/jmap/session")
            .bearer_auth(&self.token)
            .send().await.map_err(|e| format!("{e}"))?;
        let session: serde_json::Value = resp.json().await.map_err(|e| format!("{e}"))?;
        let api_url = session.get("apiUrl").and_then(|v| v.as_str())
            .ok_or("missing apiUrl")?.to_string();
        let account_id = session.get("primaryAccounts")
            .and_then(|pa| pa.get("urn:ietf:params:jmap:mail"))
            .and_then(|v| v.as_str())
            .ok_or("missing account id")?.to_string();
        Ok((api_url, account_id))
    }

    async fn search_by_subject(&self, subject: &str) -> Result<u64, String> {
        let (api_url, account_id) = self.session().await?;
        let body = json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
            "methodCalls": [[
                "Email/query",
                {
                    "accountId": account_id,
                    "filter": { "subject": subject },
                    "calculateTotal": true,
                    "limit": 5,
                },
                "R1"
            ]]
        });
        let resp = self.http.post(&api_url)
            .bearer_auth(&self.token)
            .json(&body)
            .send().await.map_err(|e| format!("{e}"))?;
        let result: serde_json::Value = resp.json().await.map_err(|e| format!("{e}"))?;
        let total = result.get("methodResponses")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|mr| mr.as_array())
            .and_then(|arr| arr.get(1))
            .and_then(|r| r.get("total"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        Ok(total)
    }

    async fn wait_for_email(&self, subject: &str, max_attempts: u32) -> bool {
        for attempt in 1..=max_attempts {
            tokio::time::sleep(Duration::from_secs(2)).await;
            match self.search_by_subject(subject).await {
                Ok(n) if n > 0 => {
                    eprintln!("  found after {attempt} attempts ({n} results)");
                    return true;
                }
                Ok(_) => eprintln!("  attempt {attempt}/{max_attempts}: not indexed yet..."),
                Err(e) => eprintln!("  attempt {attempt}/{max_attempts}: search error: {e}"),
            }
        }
        false
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Plugin loading helper
// ═══════════════════════════════════════════════════════════════════════

fn load_email_plugin(secrets: &Secrets) -> Arc<Mutex<clankers_plugin::PluginManager>> {
    // Set env vars the plugin expects (config_env mapping)
    // SAFETY: test runs single-threaded for plugin init; no concurrent readers.
    unsafe {
        std::env::set_var("FASTMAIL_API_TOKEN", &secrets.api_token);
        std::env::set_var("CLANKERS_EMAIL_FROM", &secrets.email_from);
        std::env::set_var("CLANKERS_EMAIL_ALLOWED_RECIPIENTS", "@robitzs.ch");
    }

    let plugin_dir = std::path::PathBuf::from("plugins");
    let mgr = Arc::new(Mutex::new(
        clankers_plugin::PluginManager::new(plugin_dir, None),
    ));

    {
        let mut m = mgr.lock().unwrap();
        // Discover from the plugins/ directory in the repo
        m.discover();
        // Load the email plugin WASM
        if let Err(e) = m.load_wasm("clankers-email") {
            panic!("Failed to load clankers-email WASM plugin: {e}");
        }
    }

    mgr
}

/// Dispatch a ScheduleFire DaemonEvent to plugins and return any messages.
fn dispatch_schedule_fire(
    mgr: &Arc<Mutex<clankers_plugin::PluginManager>>,
    event: &DaemonEvent,
) -> Vec<String> {
    let m = mgr.lock().unwrap();
    let event_kind = "schedule_fire";

    let payload = match event {
        DaemonEvent::ScheduleFire { schedule_id, schedule_name, payload, fire_count } => {
            json!({
                "event": "schedule_fire",
                "data": {
                    "schedule_id": schedule_id,
                    "schedule_name": schedule_name,
                    "payload": payload,
                    "fire_count": fire_count,
                }
            })
        }
        _ => return vec![],
    };

    let input = serde_json::to_string(&payload).unwrap();
    let mut messages = Vec::new();

    for info in m.list() {
        if !matches!(info.state, clankers_plugin::PluginState::Active) {
            continue;
        }
        let subscribed = info.manifest.events.iter().any(|e| e == event_kind);
        if !subscribed {
            continue;
        }
        match m.call_plugin(&info.name, "on_event", &input) {
            Ok(output) => {
                eprintln!("  plugin '{}' returned: {}", info.name, output);
                messages.push(output);
            }
            Err(e) => {
                eprintln!("  plugin '{}' error: {e}", info.name);
                messages.push(format!("ERROR: {e}"));
            }
        }
    }

    messages
}

// ═══════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════

/// One-shot schedule fires → plugin sends email → verify delivery via JMAP.
#[tokio::test]
async fn one_shot_schedule_sends_email_via_plugin() {
    let Some(secrets) = load_secrets() else {
        eprintln!("SKIP: sops secrets unavailable");
        return;
    };

    let verifier = JmapVerifier::new(secrets.api_token.clone());
    let mgr = load_email_plugin(&secrets);
    let subject = test_subject();

    // Create a one-shot schedule that fires immediately
    let fire_at = Utc::now() - chrono::Duration::seconds(1);
    let schedule_payload = json!({
        "action": "send_email",
        "to": TEST_RECIPIENT,
        "subject": subject,
        "body": format!("One-shot scheduled email test.\nFired at: {}", Utc::now()),
        "from": secrets.email_from,
    });

    let engine = ScheduleEngine::new();
    let mut rx = engine.subscribe();
    engine.add(Schedule::once("test-oneshot", fire_at, schedule_payload));

    // Tick the engine — schedule should fire
    engine.tick();
    let event = rx.try_recv().expect("schedule should fire");
    assert_eq!(event.schedule_name, "test-oneshot");
    assert_eq!(event.fire_count, 1);
    eprintln!("  schedule fired: {} (count={})", event.schedule_name, event.fire_count);

    // Convert to DaemonEvent and dispatch to plugin
    let daemon_event = DaemonEvent::ScheduleFire {
        schedule_id: event.schedule_id.0,
        schedule_name: event.schedule_name,
        payload: event.payload,
        fire_count: event.fire_count,
    };
    let messages = dispatch_schedule_fire(&mgr, &daemon_event);
    assert!(!messages.is_empty(), "plugin should respond");
    assert!(
        messages.iter().any(|m| m.contains("sent email")),
        "plugin should confirm email sent, got: {:?}", messages,
    );

    // Verify delivery
    assert!(
        verifier.wait_for_email(&subject, 10).await,
        "email should appear in JMAP search within 20s",
    );

    // One-shot should be expired
    assert!(engine.list().is_empty(), "one-shot should be GC'd");
}

/// Interval schedule fires twice → plugin sends two emails.
#[tokio::test]
async fn interval_schedule_sends_multiple_emails() {
    let Some(secrets) = load_secrets() else {
        eprintln!("SKIP: sops secrets unavailable");
        return;
    };

    let verifier = JmapVerifier::new(secrets.api_token.clone());
    let mgr = load_email_plugin(&secrets);
    let subject = test_subject();

    let schedule_payload = json!({
        "action": "send_email",
        "to": TEST_RECIPIENT,
        "subject": subject,
        "body": "Interval schedule test — fires twice.",
        "from": secrets.email_from,
    });

    let mut sched = Schedule::interval("test-interval", 1, schedule_payload);
    sched.max_fires = Some(2);
    sched.last_fired = Some(Utc::now() - chrono::Duration::seconds(100));

    let engine = ScheduleEngine::new();
    let mut rx = engine.subscribe();
    engine.add(sched);

    // First fire
    engine.tick();
    let ev1 = rx.try_recv().expect("first fire");
    assert_eq!(ev1.fire_count, 1);
    let msgs1 = dispatch_schedule_fire(&mgr, &DaemonEvent::ScheduleFire {
        schedule_id: ev1.schedule_id.0,
        schedule_name: ev1.schedule_name,
        payload: ev1.payload,
        fire_count: ev1.fire_count,
    });
    assert!(msgs1.iter().any(|m| m.contains("sent email")), "fire 1 should send");
    eprintln!("  fire 1 done");

    // Wait for interval, second fire
    tokio::time::sleep(Duration::from_secs(2)).await;
    engine.tick();
    let ev2 = rx.try_recv().expect("second fire");
    assert_eq!(ev2.fire_count, 2);
    let msgs2 = dispatch_schedule_fire(&mgr, &DaemonEvent::ScheduleFire {
        schedule_id: ev2.schedule_id.0,
        schedule_name: ev2.schedule_name,
        payload: ev2.payload,
        fire_count: ev2.fire_count,
    });
    assert!(msgs2.iter().any(|m| m.contains("sent email")), "fire 2 should send");
    eprintln!("  fire 2 done");

    // Schedule should be expired (max_fires=2)
    assert!(engine.list().is_empty(), "should expire after 2 fires");

    // Verify at least one email arrived
    assert!(
        verifier.wait_for_email(&subject, 10).await,
        "interval emails should arrive",
    );
}

/// Non-email payloads are ignored by the email plugin.
#[tokio::test]
async fn non_email_payload_ignored_by_plugin() {
    let Some(secrets) = load_secrets() else {
        eprintln!("SKIP: sops secrets unavailable");
        return;
    };

    let mgr = load_email_plugin(&secrets);

    let daemon_event = DaemonEvent::ScheduleFire {
        schedule_id: "test-id".into(),
        schedule_name: "not-email".into(),
        payload: json!({"action": "run_tests", "cmd": "cargo test"}),
        fire_count: 1,
    };

    let messages = dispatch_schedule_fire(&mgr, &daemon_event);
    // Plugin should respond but not send any email
    assert!(
        messages.iter().all(|m| !m.contains("sent email")),
        "non-email payload should not trigger send, got: {:?}", messages,
    );
}

/// Recipient outside the allowlist gets rejected.
#[tokio::test]
async fn allowlist_denies_unauthorized_recipient() {
    let Some(secrets) = load_secrets() else {
        eprintln!("SKIP: sops secrets unavailable");
        return;
    };

    let mgr = load_email_plugin(&secrets);

    let daemon_event = DaemonEvent::ScheduleFire {
        schedule_id: "test-deny".into(),
        schedule_name: "denied-email".into(),
        payload: json!({
            "action": "send_email",
            "to": "brittonrobitzsch@gmail.com",
            "subject": "[test] this should be denied",
            "body": "If you see this, the allowlist is broken.",
            "from": secrets.email_from,
        }),
        fire_count: 1,
    };

    let messages = dispatch_schedule_fire(&mgr, &daemon_event);
    eprintln!("  plugin response: {:?}", messages);
    assert!(
        messages.iter().any(|m| m.contains("not in allowlist") || m.contains("failed") || m.contains("Allowed")),
        "should be denied by allowlist, got: {:?}", messages,
    );
    assert!(
        messages.iter().all(|m| !m.contains("sent email")),
        "should NOT have sent the email, got: {:?}", messages,
    );
}

/// Full daemon integration: ScheduleEngine → run_schedule_consumer → plugin dispatch → email.
/// Exercises the real daemon code path without starting the full daemon.
#[tokio::test]
async fn daemon_schedule_consumer_dispatches_to_plugin() {
    let Some(secrets) = load_secrets() else {
        eprintln!("SKIP: sops secrets unavailable");
        return;
    };

    let verifier = JmapVerifier::new(secrets.api_token.clone());
    let mgr = load_email_plugin(&secrets);
    let subject = test_subject();

    // Build a schedule that fires immediately
    let engine = clanker_scheduler::ScheduleEngine::new()
        .with_tick_interval(Duration::from_millis(100));

    let schedule_payload = json!({
        "action": "send_email",
        "to": TEST_RECIPIENT,
        "subject": subject,
        "body": "Daemon integration test — run_schedule_consumer dispatched this.",
        "from": secrets.email_from,
    });

    let sched = Schedule::once(
        "daemon-integration-test",
        Utc::now() - chrono::Duration::seconds(1),
        schedule_payload,
    );
    engine.add(sched);

    // Fake DaemonState (no sessions needed — email goes through plugin, not session prompt)
    let state = Arc::new(tokio::sync::Mutex::new(
        clankers_controller::transport::DaemonState::new(),
    ));

    let rx = engine.subscribe();
    let cancel = tokio_util::sync::CancellationToken::new();

    // Spawn the real daemon consumer
    let consumer_cancel = cancel.clone();
    let consumer_mgr = Arc::clone(&mgr);
    let consumer_state = Arc::clone(&state);
    let consumer_handle = tokio::spawn(async move {
        clankers::modes::daemon::run_schedule_consumer(
            rx,
            consumer_state,
            Some(consumer_mgr),
            consumer_cancel,
        ).await;
    });

    // Start the tick loop — it will fire the schedule and the consumer picks it up
    let tick_handle = engine.start();

    // Wait for the email to be sent + indexed
    let found = verifier.wait_for_email(&subject, 15).await;

    // Shut down
    engine.cancel_token().cancel();
    cancel.cancel();
    let _ = tick_handle.await;
    let _ = consumer_handle.await;

    assert!(found, "daemon consumer should dispatch email via plugin within 30s");
}

/// Schedule fire with missing fields is handled gracefully.
#[tokio::test]
async fn malformed_email_payload_handled() {
    let Some(secrets) = load_secrets() else {
        eprintln!("SKIP: sops secrets unavailable");
        return;
    };

    let mgr = load_email_plugin(&secrets);

    let daemon_event = DaemonEvent::ScheduleFire {
        schedule_id: "test-id".into(),
        schedule_name: "bad-email".into(),
        payload: json!({"action": "send_email", "to": TEST_RECIPIENT}),
        // missing subject and body
        fire_count: 1,
    };

    let messages = dispatch_schedule_fire(&mgr, &daemon_event);
    // Should get an error, not crash
    assert!(
        messages.iter().any(|m| m.contains("failed") || m.contains("ERROR") || m.contains("Missing")),
        "malformed payload should produce error, got: {:?}", messages,
    );
}
