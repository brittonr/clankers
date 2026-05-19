#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;

fn main() {
    let source = fs::read_to_string("crates/clankers-engine-host/src/lib.rs").expect("read engine-host source");
    let required = [
        "MATRIX_AXIS_VALUES",
        "MATRIX_CRITICAL_TRIPLES",
        "MATRIX_CASES",
        "engine_host_feature_matrix_executes_declared_cases_and_execution_reports_axes",
        "engine_host_feature_matrix_freshness_covers_axes_and_critical_triples",
        "engine_host_feature_matrix_pairwise_policy_is_covered",
        "model_mode",
        "stop_reason",
        "tool_behavior",
        "retry_behavior",
        "cancellation_timing",
        "usage_observation",
        "stream_validity",
        "request_budget",
        "streamed_tool_calls_with_usage",
        "retryable_failures_with_cancellation",
        "budget_exhaustion_after_tool_feedback",
        "EHFM-",
    ];
    let missing: Vec<_> = required.iter().copied().filter(|needle| !source.contains(needle)).collect();
    if !missing.is_empty() {
        eprintln!("engine-host feature matrix freshness failed:");
        for item in missing {
            eprintln!("  - missing {item}");
        }
        std::process::exit(1);
    }
}
