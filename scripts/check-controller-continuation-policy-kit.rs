#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;

fn main() {
    let auto_test = fs::read_to_string("crates/clankers-controller/src/auto_test.rs")
        .expect("read controller continuation source");
    require(
        &auto_test,
        "controller_continuation_policy_kit_prioritizes_follow_ups_and_rejects_stale_effects",
        "controller continuation kit fixture missing",
    );
    require(
        &auto_test,
        "PostPromptAction::ReplayQueuedPrompt",
        "queued prompt replay priority missing",
    );
    require(
        &auto_test,
        "PostPromptAction::ContinueLoop",
        "loop continuation positive path missing",
    );
    require(
        &auto_test,
        "auto_test_in_progress",
        "auto-test recursion guard missing",
    );
    require(
        &auto_test,
        "wrong_effect_id",
        "stale effect-id negative path missing",
    );
    require(
        &auto_test,
        "assert_eq!(follow_up_ctrl.core_state, previous_state)",
        "fail-closed state preservation assertion missing",
    );

    let docs = fs::read_to_string("docs/src/reference/request-lifecycle.md")
        .expect("read request lifecycle docs");
    require(
        &docs,
        "controller-continuation-policy-kit",
        "request lifecycle docs must name the controller continuation brick",
    );
    require(
        &docs,
        "queued prompt replay",
        "request lifecycle docs must describe queued prompt replay priority",
    );
    require(
        &docs,
        "Stale follow-up effect ids",
        "request lifecycle docs must describe fail-closed stale effect behavior",
    );

    let spec = fs::read_to_string("cairn/specs/controller-continuation-policy/spec.md")
        .expect("read controller continuation spec");
    require(
        &spec,
        "Controller continuation kit proves post-prompt state transitions",
        "canonical OpenSpec requirement missing",
    );
    require(
        &spec,
        "queued user prompt",
        "canonical spec must preserve queued prompt priority",
    );
    require(
        &spec,
        "stale, duplicate, or mismatched follow-up effect id",
        "canonical spec must preserve fail-closed effect-id behavior",
    );

    println!("controller-continuation-policy-kit checker passed");
}

fn require(haystack: &str, needle: &str, message: &str) {
    assert!(haystack.contains(needle), "{message}: missing {needle:?}");
}
