#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;

fn main() {
    let source = fs::read_to_string("src/modes/batch.rs").expect("read batch runner source");
    let docs = fs::read_to_string("docs/src/getting-started/quickstart.md").expect("read quickstart docs");
    let spec = fs::read_to_string("openspec/specs/batch-trajectory-runner/spec.md")
        .expect("read batch trajectory spec");

    let required_source = [
        "batch_eval_runner_kit_fixture_validates_manifest_resume_and_redaction",
        "filter_resume_jobs(jobs, Some(&previous_manifest))",
        "render_trajectory_results(TrajectoryFormat::EvalJsonl",
        "BatchPolicyError::RemoteInputUnsupported",
        "safe_metadata_only",
    ];
    let required_docs = [
        "batch-eval-runner-kit",
        "copyable brick",
        "deterministic resume manifest",
        "fail-closed local-path validation",
    ];
    let required_spec = [
        "Batch eval kit validates deterministic manifests and resume receipts",
        "batch-eval-runner-kit.boundary",
        "batch-eval-runner-kit.evidence",
        "batch-eval-runner-kit.drift",
    ];

    assert_contains("src/modes/batch.rs", &source, &required_source);
    assert_contains("docs/src/getting-started/quickstart.md", &docs, &required_docs);
    assert_contains("openspec/specs/batch-trajectory-runner/spec.md", &spec, &required_spec);
}

fn assert_contains(path: &str, haystack: &str, needles: &[&str]) {
    let missing: Vec<_> = needles.iter().copied().filter(|needle| !haystack.contains(needle)).collect();
    if missing.is_empty() {
        return;
    }

    eprintln!("batch-eval-runner-kit drift check failed for {path}:");
    for needle in missing {
        eprintln!("  - missing {needle}");
    }
    eprintln!("owner: update batch runner tests, quickstart docs, and OpenSpec receipt evidence together");
    std::process::exit(1);
}
