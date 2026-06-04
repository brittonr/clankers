#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
blake3 = "1"
serde_json = "1"
---

use std::fs;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitCode;

use serde_json::Value;
use serde_json::json;

const ERROR_EXIT: u8 = 1;
const FIXTURE: &str = "examples/embedded-session-store/session-resume-evidence.json";
const SESSION_LEDGER_BOUNDARY: &str = "scripts/check-session-ledger-boundary.rs";
const POLICY: &str = "policy/embedded-lego/lego-contracts.json";
const DOCS: &str = "docs/src/tutorials/embedded-agent-sdk.md";
const SPEC: &str = "cairn/specs/embedded-composition-kits/spec.md";
const GREEN_LEDGER: &str = "crates/clankers-engine-host/src/session_ledger.rs";
const RUNTIME_LEDGER_ADAPTER: &str = "crates/clankers-runtime/src/ledger.rs";
const RUNTIME_SESSION: &str = "crates/clankers-runtime/src/session.rs";
const RUNTIME_PROMPT: &str = "crates/clankers-runtime/src/prompt.rs";
const RUNTIME_RUNTIME: &str = "crates/clankers-runtime/src/runtime.rs";
const DEFAULT_OUTPUT: &str = "target/embedded-sdk-release/session-resume-brick-receipt.json";
const FORBIDDEN_SOURCE_TOKENS: &[&str] = &[
    "clankers_session",
    "clankers-db",
    "clankers_db",
    "SessionManager",
    "JsonlSessionStore",
    "daemon sockets",
    "TUI/session restore logic",
];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("session-resume-brick receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("session-resume-brick check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let fixture_text = fs::read_to_string(FIXTURE).map_err(|error| format!("failed to read {FIXTURE}: {error}"))?;
    let fixture: Value =
        serde_json::from_str(&fixture_text).map_err(|error| format!("failed to parse {FIXTURE}: {error}"))?;
    require_eq(&fixture, "schema", "clankers.embedded_session_resume.evidence.v1")?;
    require_eq(&fixture, "boundary_rail", SESSION_LEDGER_BOUNDARY)?;
    validate_policy_points_at_fixture()?;
    let products = fixture
        .get("products")
        .and_then(Value::as_array)
        .ok_or_else(|| "fixture missing products array".to_string())?;
    if products.len() < 2 {
        return Err("session resume fixture must cover at least two product-shaped stores".to_string());
    }
    let mut hashed = vec![
        hash_artifact(Path::new(FIXTURE))?,
        hash_artifact(Path::new(SESSION_LEDGER_BOUNDARY))?,
        hash_artifact(Path::new(POLICY))?,
        hash_artifact(Path::new(DOCS))?,
        hash_artifact(Path::new(SPEC))?,
        hash_artifact(Path::new(GREEN_LEDGER))?,
        hash_artifact(Path::new(RUNTIME_LEDGER_ADAPTER))?,
        hash_artifact(Path::new(RUNTIME_SESSION))?,
        hash_artifact(Path::new(RUNTIME_PROMPT))?,
        hash_artifact(Path::new(RUNTIME_RUNTIME))?,
    ];
    for product in products {
        validate_product(product)?;
        let source = required_str(product, "source")?;
        hashed.push(hash_artifact(Path::new(source))?);
    }
    validate_docs_and_spec()?;
    validate_reusable_api_sources()?;
    run_runtime_resume_fixtures()?;
    let receipt = json!({
        "schema": "clankers.embedded_session_resume.receipt.v1",
        "fixture": FIXTURE,
        "products": products.iter().map(|product| json!({
            "product": required_str(product, "product").unwrap_or(""),
            "source": required_str(product, "source").unwrap_or(""),
            "store_type": required_str(product, "store_type").unwrap_or(""),
            "restored_context": product.get("restored_context").and_then(Value::as_bool).unwrap_or(false),
            "missing_session_fail_closed": product.get("missing_session_fail_closed").and_then(Value::as_bool).unwrap_or(false),
            "owns_storage_dto": product.get("owns_storage_dto").and_then(Value::as_bool).unwrap_or(false),
        })).collect::<Vec<_>>(),
        "reusable_api": {
            "ledger_module": GREEN_LEDGER,
            "runtime_compat_adapter": RUNTIME_LEDGER_ADAPTER,
            "session_runtime": RUNTIME_SESSION,
            "model_history_field": "ModelRequest.history: Vec<SessionLedgerMessage>",
            "resume_entrypoint": "Runtime::resume_session",
            "fixture": "cargo test -p clankers-runtime --lib session_resume",
            "boundary_rail": SESSION_LEDGER_BOUNDARY
        },
        "hashed_artifacts": hashed,
        "boundary": "session/message DTOs are neutral ledger entries; storage stays host-owned behind SessionStore adapters."
    });
    let output = PathBuf::from(DEFAULT_OUTPUT);
    let parent = output.parent().ok_or_else(|| format!("{} has no parent", output.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| format!("failed to encode receipt: {error}"))?;
    fs::write(&output, [bytes.as_slice(), b"\n"].concat())
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    Ok(output)
}

fn validate_policy_points_at_fixture() -> Result<(), String> {
    let policy_text = fs::read_to_string(POLICY).map_err(|error| format!("failed to read {POLICY}: {error}"))?;
    let policy: Value =
        serde_json::from_str(&policy_text).map_err(|error| format!("failed to parse {POLICY}: {error}"))?;
    if policy.pointer("/session_resume_evidence_fixture") != Some(&Value::String(FIXTURE.to_string())) {
        return Err("policy must point session_resume_evidence_fixture at the checked fixture".to_string());
    }
    Ok(())
}

fn validate_product(product: &Value) -> Result<(), String> {
    let name = required_str(product, "product")?;
    let source_path = required_str(product, "source")?;
    let source = fs::read_to_string(source_path).map_err(|error| format!("failed to read {source_path}: {error}"))?;
    for flag in ["restored_context", "missing_session_fail_closed", "owns_storage_dto"] {
        if product.get(flag).and_then(Value::as_bool) != Some(true) {
            return Err(format!("{name} must set {flag}=true"));
        }
    }
    for field in ["store_type", "missing_session_error"] {
        let expected = required_str(product, field)?;
        require_contains(&source, expected, &format!("{source_path} missing {field} `{expected}`"))?;
    }
    let dto_types = product
        .get("dto_types")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{name} missing dto_types"))?;
    for dto in dto_types {
        let dto = dto.as_str().ok_or_else(|| format!("{name} dto_types contains non-string"))?;
        require_contains(&source, dto, &format!("{source_path} missing DTO type `{dto}`"))?;
    }
    let context = product
        .get("expected_follow_up_context")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{name} missing expected_follow_up_context"))?;
    if context.len() < 3 {
        return Err(format!("{name} expected_follow_up_context must include restored context and follow-up prompt"));
    }
    for item in context {
        let text = item.as_str().ok_or_else(|| format!("{name} context contains non-string"))?;
        require_contains(&source, text, &format!("{source_path} missing restored context assertion `{text}`"))?;
    }
    for token in FORBIDDEN_SOURCE_TOKENS {
        if source.contains(token) {
            return Err(format!("{source_path} contains forbidden session-shell token `{token}`"));
        }
    }
    Ok(())
}

fn validate_docs_and_spec() -> Result<(), String> {
    let docs = fs::read_to_string(DOCS).map_err(|error| format!("failed to read {DOCS}: {error}"))?;
    for marker in [
        "session-resume-brick",
        "SessionLedgerEntry",
        "Runtime::resume_session",
        "session-resume-evidence.json",
        "scripts/check-session-resume-brick.rs",
        "scripts/check-session-ledger-boundary.rs",
        "product-owned session stores and receipt DTOs",
    ] {
        require_contains(&docs, marker, &format!("{DOCS} missing `{marker}`"))?;
    }
    let spec = fs::read_to_string(SPEC).map_err(|error| format!("failed to read {SPEC}: {error}"))?;
    for marker in [
        "Session/resume brick convergence",
        "Multiple product-shaped stores prove restored context",
        "Missing and stale sessions fail closed",
        "Reusable session ledger API is promoted",
    ] {
        require_contains(&spec, marker, &format!("{SPEC} missing `{marker}`"))?;
    }
    Ok(())
}

fn validate_reusable_api_sources() -> Result<(), String> {
    let ledger =
        fs::read_to_string(GREEN_LEDGER).map_err(|error| format!("failed to read {GREEN_LEDGER}: {error}"))?;
    for marker in [
        "pub enum SessionLedgerEntry",
        "pub struct SessionLedgerMessage",
        "pub struct SessionLedgerRecord",
        "pub struct SessionLedgerReplay",
        "pub enum SessionLedgerError",
        "pub enum SessionLedgerRole",
        "pub struct SessionLedgerReceipt",
        "pub struct SessionLedgerUsage",
        "pub struct SessionLedgerSummary",
        "pub fn replay_ledger_entries",
    ] {
        require_contains(&ledger, marker, &format!("{GREEN_LEDGER} missing `{marker}`"))?;
    }
    for token in [
        "AgentMessage",
        "DaemonEvent",
        "ConversationBlock",
        "clankers_db",
        "clankers-db",
        "JsonlSessionStore",
        "RuntimeError",
        "Utc::now",
    ] {
        if ledger.contains(token) {
            return Err(format!("{GREEN_LEDGER} contains forbidden shell token `{token}`"));
        }
    }

    let session =
        fs::read_to_string(RUNTIME_SESSION).map_err(|error| format!("failed to read {RUNTIME_SESSION}: {error}"))?;
    for marker in [
        "pub(crate) fn resume",
        "RuntimeError::SessionMissing",
        "record.replay()?.messages",
        "ledger_messages_from_engine_messages(&request.messages)",
        "replace_record_replay_messages",
    ] {
        require_contains(&session, marker, &format!("{RUNTIME_SESSION} missing `{marker}`"))?;
    }

    let prompt =
        fs::read_to_string(RUNTIME_PROMPT).map_err(|error| format!("failed to read {RUNTIME_PROMPT}: {error}"))?;
    require_contains(
        &prompt,
        "pub history: Vec<SessionLedgerMessage>",
        &format!("{RUNTIME_PROMPT} missing model request history field"),
    )?;

    let runtime =
        fs::read_to_string(RUNTIME_RUNTIME).map_err(|error| format!("failed to read {RUNTIME_RUNTIME}: {error}"))?;
    require_contains(&runtime, "pub async fn resume_session", "runtime missing resume_session entrypoint")?;
    Ok(())
}

fn run_runtime_resume_fixtures() -> Result<(), String> {
    let status = Command::new("cargo")
        .env("RUSTC_WRAPPER", "")
        .args(["test", "-p", "clankers-runtime", "--lib", "session_resume"])
        .status()
        .map_err(|error| format!("failed to run clankers-runtime session_resume fixtures: {error}"))?;
    if !status.success() {
        return Err(format!("clankers-runtime session_resume fixtures failed with status {status}"));
    }
    Ok(())
}

fn require_eq(value: &Value, field: &str, expected: &str) -> Result<(), String> {
    match value.get(field).and_then(Value::as_str) {
        Some(actual) if actual == expected => Ok(()),
        Some(actual) => Err(format!("field `{field}` expected `{expected}`, got `{actual}`")),
        None => Err(format!("missing string field `{field}`")),
    }
}

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| format!("missing non-empty string field `{field}`"))
}

fn require_contains(haystack: &str, needle: &str, message: &str) -> Result<(), String> {
    if haystack.contains(needle) {
        Ok(())
    } else {
        Err(message.to_string())
    }
}

fn hash_artifact(path: &Path) -> Result<Value, String> {
    let mut file = fs::File::open(path).map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut bytes = 0u64;
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        bytes += read as u64;
        hasher.update(&buffer[..read]);
    }
    Ok(json!({
        "path": path.to_string_lossy(),
        "bytes": bytes,
        "blake3": hasher.finalize().to_hex().to_string(),
    }))
}
