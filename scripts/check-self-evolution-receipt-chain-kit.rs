#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;

fn main() {
    let self_evolution = fs::read_to_string("src/self_evolution.rs").expect("read self-evolution tests");
    require(
        &self_evolution,
        "self_evolution_approval_records_confirmation_receipt_without_applying",
        "approval receipt chain positive fixture missing",
    );
    require(
        &self_evolution,
        "self_evolution_application_preflight_validates_without_mutation",
        "application preflight positive fixture missing",
    );
    require(
        &self_evolution,
        "self_evolution_application_rejects_stale_target_before_mutation",
        "stale target fail-closed fixture missing",
    );
    require(
        &self_evolution,
        "self_evolution_application_rejects_mismatched_or_applied_approval",
        "mismatched approval fail-closed fixture missing",
    );
    require(
        &self_evolution,
        "self_evolution_application_live_replace_writes_backup_receipt_and_target",
        "live application backup receipt fixture missing",
    );

    let validation = fs::read_to_string("src/self_evolution/validation.rs").expect("read self-evolution validation");
    require(
        &validation,
        "validate_application_receipt_chain",
        "application receipt-chain validator missing",
    );
    require(
        &validation,
        "validate_matching_approval",
        "approval/run receipt matching validator missing",
    );
    require(
        &validation,
        "target artifact changed since the run receipt was created",
        "stale target diagnostic missing",
    );
    require(
        &validation,
        "candidate artifact hash does not match the run receipt",
        "candidate hash guard missing",
    );

    let docs = fs::read_to_string("docs/src/reference/request-lifecycle.md")
        .expect("read request lifecycle docs");
    require(
        &docs,
        "self-evolution-receipt-chain-kit",
        "request lifecycle docs must name self-evolution receipt chain brick",
    );
    require(
        &docs,
        "run → approval → application → rollback",
        "request lifecycle docs must describe receipt chain order",
    );
    require(
        &docs,
        "stale target hashes, mismatched approval receipts, missing candidates, unsupported apply modes, and already-applied approvals fail closed",
        "request lifecycle docs must describe fail-closed chain guards",
    );

    let spec = fs::read_to_string("cairn/specs/self-evolution-control/spec.md")
        .expect("read self-evolution spec");
    require(
        &spec,
        "Self-evolution receipt chain kit proves gated artifact promotion",
        "canonical Cairn requirement missing",
    );
    require(
        &spec,
        "run receipt, approval receipt, application receipt, and rollback receipt",
        "canonical spec must require explicit receipt chain",
    );
    require(
        &spec,
        "stale target hash, mismatched approval, missing candidate artifact, unsupported apply mode, or already-applied approval",
        "canonical spec must require fail-closed chain guards",
    );

    println!("self-evolution-receipt-chain-kit checker passed");
}

fn require(haystack: &str, needle: &str, message: &str) {
    assert!(haystack.contains(needle), "{message}: missing {needle:?}");
}
