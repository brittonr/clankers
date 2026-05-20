#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;

fn main() {
    let process_jobs = fs::read_to_string("crates/clankers-runtime/src/process_jobs.rs")
        .expect("read process jobs runtime");
    let process_tool = fs::read_to_string("src/tools/process.rs").expect("read process tool");
    require(
        &process_jobs,
        "process_job_profile_kit_validates_manifest_policy_identity_and_redaction",
        "process job profile kit fixture missing",
    );
    require(
        &process_jobs,
        "ProjectProcessJobProfiles::from_json_str",
        "profile manifest parser missing",
    );
    require(
        &process_jobs,
        "profile manifest parses without contacting a backend",
        "positive fixture must prove parsing/resolution is pure",
    );
    require(
        &process_jobs,
        "valid profile resolves to backend-neutral start spec",
        "positive fixture must prove backend-neutral resolution",
    );
    require(
        &process_jobs,
        "secret env keys reject before backend dispatch",
        "negative fixture must reject secret env keys before backend dispatch",
    );
    require(
        &process_jobs,
        "ProcessJobIdentityEnvelope::for_start_request",
        "identity envelope fixture missing",
    );
    require(
        &process_jobs,
        "profile_manifest_sources_resolve_by_deterministic_precedence",
        "deterministic manifest precedence fixture missing",
    );
    require(
        &process_jobs,
        "profile_manifest_sources_fail_closed_on_same_precedence_duplicates",
        "same-precedence duplicate profile denial fixture missing",
    );
    require(
        &process_jobs,
        "profile_policy_rejects_paths_resources_and_unsupported_manifest_versions",
        "path/resource/schema negative fixture missing",
    );
    require(
        &process_jobs,
        "ProjectProcessJobProfileValidationError",
        "typed profile validation error missing",
    );
    require(
        &process_jobs,
        "ProcessJobProfileReceiptMetadata",
        "safe profile receipt metadata missing",
    );
    require(
        &process_tool,
        "ProcessJobProfileReceiptMetadata::from_metadata(&request.metadata)",
        "process tool must project safe profile metadata into start receipts",
    );
    require(
        &process_tool,
        "native_process_job_service_preserves_default_start_list_wait_flow",
        "native service profile receipt/list regression missing",
    );
    require(
        &process_jobs,
        "PROCESS_JOB_PROFILE_METADATA_SOURCE",
        "safe profile source metadata missing",
    );
    require(
        &process_jobs,
        "PROCESS_JOB_REDACTED",
        "redaction assertion missing",
    );

    let docs = fs::read_to_string("docs/src/reference/process-jobs.md")
        .expect("read process job docs");
    require(
        &docs,
        "process-job-profile-kit",
        "process jobs docs must name process-job-profile-kit",
    );
    require(
        &docs,
        "Resolving a profile is pure",
        "process jobs docs must state profile resolution has no backend dispatch",
    );
    require(
        &docs,
        "Secret-like environment keys such as `APP_TOKEN`, `APP_SECRET`, or `APP_KEY` fail closed before backend dispatch",
        "process jobs docs must state secret env denial",
    );
    require(
        &docs,
        "Manifest discovery is deterministic and explicit",
        "process jobs docs must state deterministic manifest precedence",
    );
    require(
        &docs,
        "Duplicate profile names at the same precedence fail closed",
        "process jobs docs must state duplicate source denial",
    );
    require(
        &docs,
        "Safe profile identity metadata is copied into the resolved start request",
        "process jobs docs must state safe profile metadata projection",
    );
    require(
        &docs,
        "scripts/check-process-job-profile-kit.rs",
        "process jobs docs must name the drift rail",
    );

    let spec = fs::read_to_string("openspec/specs/durable-process-jobs/spec.md")
        .expect("read durable process jobs spec");
    require(
        &spec,
        "Process job profile kit validates backend-neutral job manifests",
        "canonical OpenSpec requirement missing",
    );
    require(
        &spec,
        "resolving a profile produces a backend-neutral start request without spawning a process",
        "canonical spec must require pure profile resolution",
    );
    require(
        &spec,
        "disallowed backend, malformed command shape, secret-like environment key, resource limit above policy, disallowed cwd, disallowed writable path, or ambiguous manifest source",
        "canonical spec must require fail-closed policy cases",
    );
    require(
        &spec,
        "disallowed writable path",
        "canonical spec must require writable path denial",
    );

    println!("process-job-profile-kit checker passed");
}

fn require(haystack: &str, needle: &str, message: &str) {
    assert!(haystack.contains(needle), "{message}: missing {needle:?}");
}
