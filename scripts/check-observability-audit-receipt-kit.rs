#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;

fn main() {
    let audit = fs::read_to_string("crates/clankers-controller/src/audit.rs")
        .expect("read audit source");
    require(
        &audit,
        "ObservabilityAuditReceipt",
        "audit receipt type missing",
    );
    require(
        &audit,
        "observability_audit_receipt_kit_bounds_and_redacts_tool_state",
        "focused observability audit receipt fixture missing",
    );
    require(
        &audit,
        "pending_count: self.pending.len().min(MAX_PENDING_CALLS)",
        "bounded pending count receipt guard missing",
    );
    require(
        &audit,
        "pending_over_limit: self.pending.len() > MAX_PENDING_CALLS",
        "over-limit receipt diagnostic missing",
    );
    require(
        &audit,
        "!receipt_json.contains(\"SECRET_TOKEN\")",
        "redaction assertion for secret token missing",
    );
    require(
        &audit,
        "!receipt_json.contains(\"raw tool output\")",
        "redaction assertion for raw tool output missing",
    );

    let docs = fs::read_to_string("docs/src/reference/request-lifecycle.md")
        .expect("read request lifecycle docs");
    require(
        &docs,
        "observability-audit-receipt-kit",
        "request lifecycle docs must name the observability audit receipt brick",
    );
    require(
        &docs,
        "bounded counts",
        "request lifecycle docs must describe bounded receipt counts",
    );
    require(
        &docs,
        "no raw tool names, call ids, tool output, prompts, provider payloads, credentials, authorization headers, OAuth tokens, raw tool arguments, or secret environment values",
        "request lifecycle docs must describe redaction boundary",
    );

    let spec = fs::read_to_string("openspec/specs/session-metrics-capture/spec.md")
        .expect("read session metrics spec");
    require(
        &spec,
        "Observability kit emits bounded redacted receipts",
        "canonical OpenSpec requirement missing",
    );
    require(
        &spec,
        "bounded counts and booleans",
        "canonical spec must require bounded receipt shape",
    );
    require(
        &spec,
        "raw tool names, call ids, prompts, provider payloads, credentials, authorization headers, OAuth tokens, raw tool arguments, tool output, or secret environment values",
        "canonical spec must require redaction boundary",
    );

    println!("observability-audit-receipt-kit checker passed");
}

fn require(haystack: &str, needle: &str, message: &str) {
    assert!(haystack.contains(needle), "{message}: missing {needle:?}");
}
