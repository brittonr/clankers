#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

const FIXTURE_ROOT: &str = "scripts/fixtures/openspec-review-gates";
const ERROR_EXIT: u8 = 1;

const GUIDANCE_PATH: &str = "docs/src/reference/openspec-review-gates.md";
const OPERATOR_GUIDE_PATH: &str = "docs/src/reference/openspec-review-gates.md";
const FLAKE_PATH: &str = "flake.nix";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedStatus {
    Pass,
    Fail,
}

#[derive(Debug)]
struct FixtureExpectation {
    status: ExpectedStatus,
    diagnostics: Vec<String>,
}

#[derive(Debug)]
struct FixtureReport {
    name: String,
    diagnostics: Vec<String>,
}

#[derive(Debug)]
struct ContractCategory {
    code: &'static str,
    label: &'static str,
    terms: &'static [&'static str],
}

#[derive(Debug)]
struct DesignCategory {
    code: &'static str,
    label: &'static str,
    trigger_terms: &'static [&'static str],
    required_terms: &'static [&'static str],
}

#[derive(Debug)]
struct SpecCategory {
    code: &'static str,
    label: &'static str,
    trigger_terms: &'static [&'static str],
    required_terms: &'static [&'static str],
}

const CONTRACT_CATEGORIES: &[ContractCategory] = &[
    ContractCategory {
        code: "missing-deterministic-request-shape-task",
        label: "request shape",
        terms: &["request shape", "request header", "request body", "exact request"],
    },
    ContractCategory {
        code: "missing-deterministic-stream-boundary-task",
        label: "stream boundary",
        terms: &[
            "stream boundary",
            "stream boundaries",
            "stream event boundary",
            "stream event boundaries",
            "sse",
            "event-stream",
        ],
    },
    ContractCategory {
        code: "missing-deterministic-retry-policy-task",
        label: "retry policy",
        terms: &["retry policy", "retry count", "retry attempt", "backoff"],
    },
    ContractCategory {
        code: "missing-deterministic-redaction-policy-task",
        label: "redaction policy",
        terms: &["redaction", "secret", "credential"],
    },
    ContractCategory {
        code: "missing-deterministic-receipt-field-task",
        label: "receipt field",
        terms: &["receipt field", "receipt metadata", "receipt schema"],
    },
    ContractCategory {
        code: "missing-deterministic-discovery-visibility-task",
        label: "discovery visibility",
        terms: &["discovery visibility", "catalog visibility", "discoverable"],
    },
    ContractCategory {
        code: "missing-default-override-request-shape-task",
        label: "request default/override",
        terms: &[
            "default/override",
            "default and override",
            "verbosity",
            "text={\"verbosity\":\"medium\"}",
        ],
    },
    ContractCategory {
        code: "missing-active-account-task",
        label: "active account persistence",
        terms: &[
            "active account",
            "mark the requested account active",
            "requested account active",
        ],
    },
    ContractCategory {
        code: "missing-entitlement-probe-retry-task",
        label: "entitlement probe retry fixture",
        terms: &[
            "entitlement probe",
            "probe retry",
            "probe retries",
            "refresh-retry probe",
            "401 refresh-retry probe",
        ],
    },
    ContractCategory {
        code: "missing-tool-call-delta-boundary-task",
        label: "tool-call delta stream boundary",
        terms: &[
            "function_call_arguments.delta",
            "tool-call delta",
            "tool call delta",
            "inputjsondelta",
        ],
    },
    ContractCategory {
        code: "missing-auto-fix-task",
        label: "auto-fix remediation path",
        terms: &["auto-fix", "autofix", "automatic fix", "fix-it"],
    },
];

const DESIGN_CATEGORIES: &[DesignCategory] = &[
    DesignCategory {
        code: "missing-reasoning-signature-design",
        label: "reasoning signature retention",
        trigger_terms: &["reasoning signature", "signature retention"],
        required_terms: &["store", "reuse", "later turn"],
    },
    DesignCategory {
        code: "missing-retry-policy-design",
        label: "retry policy bounds",
        trigger_terms: &["retry policy", "429", "5xx", "401 refresh", "backoff"],
        required_terms: &["3 retries", "1s/2s/4s", "exactly one 401", "one refresh"],
    },
    DesignCategory {
        code: "missing-verification-plan-design",
        label: "scenario-complete verification plan",
        trigger_terms: &[
            "verification plan",
            "acceptance evidence",
            "proactive refresh",
            "provider-scoped status",
            "discovery hiding",
        ],
        required_terms: &[
            "proactive refresh",
            "401",
            "429",
            "provider-scoped status",
            "discovery hiding",
        ],
    },
];

const SPEC_CATEGORIES: &[SpecCategory] = &[
    SpecCategory {
        code: "missing-omitted-provider-default-spec",
        label: "omitted-provider default behavior",
        trigger_terms: &[
            "omitted-provider",
            "omitted provider",
            "provider omitted",
            "anthropic defaults",
        ],
        required_terms: &["omitted", "provider", "anthropic"],
    },
    SpecCategory {
        code: "missing-malformed-account-claim-spec",
        label: "malformed account-claim behavior",
        trigger_terms: &[
            "missing or malformed claim",
            "malformed claim",
            "chatgpt_account_id",
            "chatgpt-account-id",
        ],
        required_terms: &["malformed", "claim", "chatgpt"],
    },
    SpecCategory {
        code: "missing-provider-scoped-status-spec",
        label: "provider-scoped status behavior",
        trigger_terms: &["status --provider"],
        required_terms: &["status", "provider", "openai-codex"],
    },
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: openspec review-gate fixtures passed");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    let fixture_root = Path::new(FIXTURE_ROOT);
    let mut fixture_dirs = fixture_directories(fixture_root)?;
    fixture_dirs.sort();
    require(!fixture_dirs.is_empty(), "no openspec review-gate fixtures found")?;

    let mut checked = 0usize;
    for fixture_dir in fixture_dirs {
        let expectation = read_expectation(&fixture_dir)?;
        let report = evaluate_fixture(&fixture_dir)?;
        compare_report(&report, &expectation)?;
        println!("fixture {}: {:?} {:?}", report.name, expectation.status, report.diagnostics);
        checked += 1;
    }

    require(checked >= 5, "expected at least five review-gate fixtures")?;
    verify_guidance_and_wiring()?;
    Ok(())
}

fn verify_guidance_and_wiring() -> Result<(), String> {
    let guidance =
        fs::read_to_string(GUIDANCE_PATH).map_err(|error| format!("failed to read {GUIDANCE_PATH}: {error}"))?;
    for required in [
        "request shape",
        "stream boundaries",
        "retry policy",
        "security/redaction policy",
        "receipt fields",
        "discovery visibility",
        "default/override",
        "active account",
        "entitlement probe",
        "tool-call delta",
        "auto-fix remediation path",
        "reasoning signature retention",
        "retry policy bounds",
        "scenario-complete verification plan",
        "omitted-provider default behavior",
        "malformed account-claim behavior",
        "provider-scoped status behavior",
        "fixture/helper/command",
        "scripts/check-openspec-review-gates.rs",
        "Artifact-Type: oracle-checkpoint",
    ] {
        require(guidance.contains(required), &format!("{GUIDANCE_PATH} must document {required:?}"))?;
    }

    let operator_guide = fs::read_to_string(OPERATOR_GUIDE_PATH)
        .map_err(|error| format!("failed to read {OPERATOR_GUIDE_PATH}: {error}"))?;
    for required in [
        "scripts/check-openspec-review-gates.rs",
        FIXTURE_ROOT,
        "missing-deterministic-request-shape-task",
        "missing-deterministic-stream-boundary-task",
        "missing-deterministic-retry-policy-task",
        "missing-default-override-request-shape-task",
        "missing-active-account-task",
        "missing-entitlement-probe-retry-task",
        "missing-tool-call-delta-boundary-task",
        "missing-auto-fix-task",
        "missing-reasoning-signature-design",
        "missing-retry-policy-design",
        "missing-verification-plan-design",
        "missing-omitted-provider-default-spec",
        "missing-malformed-account-claim-spec",
        "missing-provider-scoped-status-spec",
        "missing-oracle-checkpoint-task",
        "invalid-oracle-checkpoint-evidence",
        "Artifact-Type: oracle-checkpoint",
        "Task-ID:",
        "Covers:",
        "Reviewed-Evidence:",
        "Decision:",
        "Follow-Up:",
    ] {
        require(operator_guide.contains(required), &format!("{OPERATOR_GUIDE_PATH} must document {required:?}"))?;
    }

    let flake = fs::read_to_string(FLAKE_PATH).map_err(|error| format!("failed to read {FLAKE_PATH}: {error}"))?;
    require(
        flake.contains("openspec-review-gates"),
        "flake.nix must expose checks.<system>.openspec-review-gates",
    )?;
    require(
        flake.contains("check-openspec-review-gates.rs"),
        "flake.nix openspec review gate check must run the Rust checker",
    )?;

    Ok(())
}

fn fixture_directories(root: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(root).map_err(|error| format!("read {}: {error}", root.display()))?;
    let mut dirs = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("read fixture dir entry: {error}"))?;
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    Ok(dirs)
}

fn read_expectation(fixture_dir: &Path) -> Result<FixtureExpectation, String> {
    let text = read_required(fixture_dir, "expect.txt")?;
    let mut status = None;
    let mut diagnostics = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(value) = line.strip_prefix("status=") {
            status = Some(match value.trim() {
                "pass" => ExpectedStatus::Pass,
                "fail" => ExpectedStatus::Fail,
                other => return Err(format!("{} has unknown status {other:?}", fixture_dir.display())),
            });
            continue;
        }
        if let Some(value) = line.strip_prefix("diagnostic=") {
            diagnostics.push(value.trim().to_owned());
            continue;
        }
        return Err(format!("{} has unsupported expectation line {line:?}", fixture_dir.display()));
    }
    let status = status.ok_or_else(|| format!("{} missing status=...", fixture_dir.display()))?;
    Ok(FixtureExpectation { status, diagnostics })
}

fn evaluate_fixture(fixture_dir: &Path) -> Result<FixtureReport, String> {
    let name = fixture_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("fixture path has no UTF-8 name: {}", fixture_dir.display()))?
        .to_owned();
    let proposal = read_optional(fixture_dir, "proposal.md")?;
    let design = read_optional(fixture_dir, "design.md")?;
    let spec = read_optional(fixture_dir, "spec.md")?;
    let tasks = read_required(fixture_dir, "tasks.md")?;
    let artifact_text = format!("{proposal}\n{design}\n{spec}");
    let lower_artifact_text = artifact_text.to_lowercase();
    let lower_design_source_text = format!("{proposal}\n{spec}").to_lowercase();
    let lower_spec_source_text = format!("{proposal}\n{design}").to_lowercase();
    let lower_design = design.to_lowercase();
    let lower_spec = spec.to_lowercase();
    let lower_tasks = tasks.to_lowercase();
    let task_lines = task_lines(&tasks);

    let mut diagnostics = Vec::new();
    for category in CONTRACT_CATEGORIES {
        if category_required(&lower_artifact_text, category) && !category_satisfied(category, &task_lines, &lower_tasks)
        {
            diagnostics.push(format!(
                "{}: deterministic contract {:?} is not traced to an explicit fixture/helper/command task",
                category.code, category.label
            ));
        }
    }

    for category in DESIGN_CATEGORIES {
        if design_category_required(&lower_design_source_text, category)
            && !design_category_satisfied(&lower_design, category)
        {
            diagnostics.push(format!(
                "{}: design contract {:?} is not defined with concrete storage/policy/verification details",
                category.code, category.label
            ));
        }
    }

    for category in SPEC_CATEGORIES {
        if spec_category_required(&lower_spec_source_text, category) && !spec_category_satisfied(&lower_spec, category)
        {
            diagnostics.push(format!(
                "{}: spec contract {:?} is not encoded as an explicit delta requirement/scenario",
                category.code, category.label
            ));
        }
    }

    if oracle_required(&lower_artifact_text) {
        let oracle_tasks = oracle_tasks(&task_lines);
        if oracle_tasks.is_empty() {
            diagnostics
                .push("missing-oracle-checkpoint-task: repeated human/oracle finding requires an H# task".to_owned());
        } else {
            for task in oracle_tasks {
                match extract_evidence_path(task) {
                    Some(evidence_path) => validate_oracle_evidence(fixture_dir, &evidence_path, &mut diagnostics),
                    None => diagnostics.push(format!(
                        "missing-oracle-checkpoint-evidence: oracle task lacks [evidence=...] marker: {task}"
                    )),
                }
            }
        }
    }

    diagnostics.sort();
    diagnostics.dedup();
    Ok(FixtureReport { name, diagnostics })
}

fn category_required(text: &str, category: &ContractCategory) -> bool {
    category.terms.iter().any(|term| text.contains(term))
}

fn design_category_required(text: &str, category: &DesignCategory) -> bool {
    category.trigger_terms.iter().any(|term| text.contains(term))
}

fn design_category_satisfied(design: &str, category: &DesignCategory) -> bool {
    category.required_terms.iter().all(|term| design.contains(term))
}

fn spec_category_required(text: &str, category: &SpecCategory) -> bool {
    category.trigger_terms.iter().any(|term| text.contains(term))
}

fn spec_category_satisfied(spec: &str, category: &SpecCategory) -> bool {
    category.required_terms.iter().all(|term| spec.contains(term))
}

fn category_satisfied(category: &ContractCategory, task_lines: &[String], lower_tasks: &str) -> bool {
    let category_is_named = category.terms.iter().any(|term| lower_tasks.contains(term));
    if !category_is_named {
        return false;
    }
    task_lines.iter().any(|line| {
        let lower = line.to_lowercase();
        category.terms.iter().any(|term| lower.contains(term)) && has_concrete_verification_marker(&lower)
    })
}

fn has_concrete_verification_marker(line: &str) -> bool {
    line.contains("[covers=")
        && (line.contains("fixture")
            || line.contains("helper")
            || line.contains("command")
            || line.contains("[evidence=")
            || line.contains("golden")
            || line.contains("scripts/"))
}

fn oracle_required(text: &str) -> bool {
    (text.contains("human-routed") || text.contains("oracle") || text.contains("human/oracle"))
        && (text.contains("repeated") || text.contains("review finding"))
}

fn task_lines(tasks: &str) -> Vec<String> {
    tasks
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("- [") && line.contains(']'))
        .map(ToOwned::to_owned)
        .collect()
}

fn oracle_tasks(task_lines: &[String]) -> Vec<&String> {
    task_lines
        .iter()
        .filter(|line| {
            let lower = line.to_lowercase();
            contains_h_task_id(line) || lower.contains("oracle-checkpoint") || lower.contains("human-routed")
        })
        .collect()
}

fn contains_h_task_id(line: &str) -> bool {
    line.split(|character: char| !character.is_ascii_alphanumeric()).any(|token| {
        token.len() >= 2 && token.starts_with('H') && token[1..].chars().all(|character| character.is_ascii_digit())
    })
}

fn extract_evidence_path(task: &str) -> Option<String> {
    let start = task.find("[evidence=")? + "[evidence=".len();
    let rest = &task[start..];
    let end = rest.find(']')?;
    Some(rest[..end].to_owned())
}

fn validate_oracle_evidence(fixture_dir: &Path, evidence_path: &str, diagnostics: &mut Vec<String>) {
    let path = fixture_dir.join(evidence_path);
    let evidence = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            diagnostics.push(format!("missing-oracle-checkpoint-evidence: failed to read {}: {error}", path.display()));
            return;
        }
    };
    for required in [
        "Artifact-Type: oracle-checkpoint",
        "Task-ID:",
        "Covers:",
        "Reviewed-Evidence:",
        "Decision:",
        "Follow-Up:",
    ] {
        if !evidence.contains(required) {
            diagnostics.push(format!("invalid-oracle-checkpoint-evidence: {} missing {required:?}", path.display()));
        }
    }
}

fn compare_report(report: &FixtureReport, expectation: &FixtureExpectation) -> Result<(), String> {
    let passed = report.diagnostics.is_empty();
    match (expectation.status, passed) {
        (ExpectedStatus::Pass, false) => {
            return Err(format!("fixture {} expected pass but got diagnostics: {:?}", report.name, report.diagnostics));
        }
        (ExpectedStatus::Fail, true) => return Err(format!("fixture {} expected fail but passed", report.name)),
        _ => {}
    }

    for expected in &expectation.diagnostics {
        require(
            report.diagnostics.iter().any(|diagnostic| diagnostic.contains(expected)),
            &format!(
                "fixture {} missing expected diagnostic substring {expected:?}; got {:?}",
                report.name, report.diagnostics
            ),
        )?;
    }
    Ok(())
}

fn read_required(dir: &Path, name: &str) -> Result<String, String> {
    let path = dir.join(name);
    fs::read_to_string(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

fn read_optional(dir: &Path, name: &str) -> Result<String, String> {
    let path = dir.join(name);
    match fs::read_to_string(&path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(format!("failed to read {}: {error}", path.display())),
    }
}

fn require(condition: bool, message: &str) -> Result<(), String> {
    if condition { Ok(()) } else { Err(message.to_owned()) }
}
