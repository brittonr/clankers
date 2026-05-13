use std::fs;
use std::path::Path;

use chrono::SecondsFormat;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use super::SelfEvolutionExecutor;
use super::receipts::*;
use super::validation::artifact_identity;
use super::validation::read_target_body;
use super::validation::sha256_hex;
use super::validation::validate_run_options;

pub fn run_self_evolution_dry_run(
    options: &SelfEvolutionRunOptions,
    executor: &mut impl SelfEvolutionExecutor,
) -> std::result::Result<SelfEvolutionRunReceipt, String> {
    validate_run_options(options)?;

    let run_id = format!("self-evolution-{}", Uuid::new_v4());
    let output_dir = options.candidate_output.join(&run_id);
    fs::create_dir_all(&output_dir).map_err(|err| format!("failed to create candidate output dir: {err}"))?;

    let baseline_body = read_target_body(&options.target)?;
    let target = artifact_identity(&options.target, baseline_body.as_deref())?;
    let candidate_body = options.candidate_body.clone().unwrap_or_else(|| baseline_body.clone().unwrap_or_default());
    let candidate_path = output_dir.join("candidate.txt");
    fs::write(&candidate_path, &candidate_body).map_err(|err| format!("failed to write isolated candidate: {err}"))?;

    let candidate_hash = sha256_hex(candidate_body.as_bytes());
    let baseline_hash = baseline_body.as_deref().map(|body| sha256_hex(body.as_bytes()));
    let changed = baseline_hash.as_deref() != Some(candidate_hash.as_str());
    let eval = deterministic_evaluation(changed, options.simulate_eval_failure);
    let corpus = load_eval_corpus_manifest(options.corpus_manifest.as_deref())?;

    let prompt_receipt = executor.submit_tool(
        options.session_id.as_deref(),
        "send_prompt",
        json!({
            "prompt_len": self_evolution_prompt_len(&target, &options.baseline_command),
            "purpose": "self_evolution_candidate_review",
        }),
    );
    let history_receipt = executor.submit_tool(
        options.session_id.as_deref(),
        "session_history",
        json!({ "purpose": "self_evolution_receipt_evidence" }),
    );

    let readiness =
        readiness_report(options, corpus.as_ref(), &eval, changed, &[prompt_receipt.clone(), history_receipt.clone()]);
    let recommendation = promotion_recommendation_from_readiness(&readiness, &eval, changed);

    let candidate = CandidateRecord {
        output_dir: output_dir.display().to_string(),
        artifact_path: candidate_path.display().to_string(),
        sha256: candidate_hash,
        bytes: candidate_body.len() as u64,
        changed_from_baseline: changed,
        score: eval.candidate_score,
        status: eval.candidate_status.clone(),
        evidence: candidate_evidence(eval.failed),
    };

    let baseline = EvaluationRecord {
        command: options.baseline_command.clone(),
        score: eval.baseline_score,
        status: eval.baseline_status,
        evidence: baseline_evidence(eval.failed),
    };

    let receipt_status = if eval.failed {
        "completed_with_failed_evaluation"
    } else {
        "completed"
    };

    let receipt = SelfEvolutionRunReceipt {
        source: "self_evolution_dry_run".to_string(),
        run_id,
        status: receipt_status.to_string(),
        dry_run: true,
        target,
        baseline,
        candidate,
        mcp_receipts: vec![prompt_receipt, history_receipt],
        recommendation,
        readiness,
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
    };

    let receipt_path = output_dir.join("receipt.json");
    let receipt_json = serde_json::to_string_pretty(&receipt).map_err(|err| err.to_string())?;
    fs::write(&receipt_path, receipt_json).map_err(|err| format!("failed to write receipt: {err}"))?;

    Ok(receipt)
}

pub fn load_eval_corpus_manifest(path: Option<&Path>) -> std::result::Result<Option<EvalCorpusManifest>, String> {
    let Some(path) = path else {
        return Ok(None);
    };
    let body = fs::read_to_string(path).map_err(|err| format!("failed to read eval corpus manifest: {err}"))?;
    let manifest: EvalCorpusManifest =
        serde_json::from_str(&body).map_err(|err| format!("failed to parse eval corpus manifest: {err}"))?;
    validate_eval_corpus_manifest(&manifest)?;
    Ok(Some(manifest))
}

fn validate_eval_corpus_manifest(manifest: &EvalCorpusManifest) -> std::result::Result<(), String> {
    if manifest.version == 0 {
        return Err("eval corpus manifest version must be greater than zero".to_string());
    }
    if manifest.targets.is_empty() {
        return Err("eval corpus manifest must declare at least one target".to_string());
    }
    if manifest.cases.is_empty() {
        return Err("eval corpus manifest must declare at least one case".to_string());
    }
    if manifest.redaction_policy.trim().is_empty() {
        return Err("eval corpus manifest must declare a redaction policy".to_string());
    }
    for case in &manifest.cases {
        if case.id.trim().is_empty() || case.oracle_command.trim().is_empty() {
            return Err("eval corpus cases require id and oracle_command".to_string());
        }
    }
    Ok(())
}

fn readiness_report(
    options: &SelfEvolutionRunOptions,
    corpus: Option<&EvalCorpusManifest>,
    eval: &DeterministicEvaluation,
    changed: bool,
    receipts: &[McpOrchestrationReceipt],
) -> SelfEvolutionReadinessReport {
    let profile = normalized_profile(&options.production_profile);
    let daemon_session_observable = receipts.iter().all(|receipt| receipt.submitted) && !receipts.is_empty();
    let improvement = eval.candidate_score - eval.baseline_score;
    let threshold = corpus.map(|manifest| manifest.min_improvement).unwrap_or(0.0);
    let threshold_passed = !eval.failed && improvement >= threshold;
    let regression_budget_passed = corpus.map(|manifest| manifest.regression_budget == 0).unwrap_or(false);
    let unchanged_candidate_control_passed = changed;
    let corpus_cases = corpus.map(|manifest| manifest.cases.len()).unwrap_or(0);

    let mut reasons = Vec::new();
    let label = if eval.failed {
        reasons.push("evaluation failed".to_string());
        "blocked"
    } else if profile == "dry_run_only" {
        reasons.push("dry-run profile does not claim production evidence".to_string());
        "dry_run_only"
    } else if corpus.is_none() {
        reasons.push("production profile requires a valid local eval corpus manifest".to_string());
        "blocked"
    } else if !daemon_session_observable {
        reasons.push("run did not record observable daemon/session receipts".to_string());
        "blocked"
    } else if !unchanged_candidate_control_passed {
        reasons.push("candidate was unchanged from baseline; positive deltas are treated as noise".to_string());
        "controlled_dogfood"
    } else if !threshold_passed {
        reasons.push("minimum improvement threshold was not met".to_string());
        "controlled_dogfood"
    } else if !regression_budget_passed {
        reasons.push("regression budget was not fully satisfied".to_string());
        "controlled_dogfood"
    } else {
        reasons.push(
            "corpus, threshold, regression budget, unchanged-control, and session observability gates passed"
                .to_string(),
        );
        "promotion_eligible"
    };

    SelfEvolutionReadinessReport {
        label: label.to_string(),
        profile,
        corpus_manifest_path: options.corpus_manifest.as_ref().map(|path| path.display().to_string()),
        corpus_cases,
        threshold_passed,
        regression_budget_passed,
        unchanged_candidate_control_passed,
        daemon_session_observable,
        evidence_refs: receipts.iter().map(|receipt| format!("{}:{}", receipt.tool, receipt.status)).collect(),
        reasons,
        known_limitations: vec![
            "deterministic local evaluator; no active artifacts are mutated by run".to_string(),
            "human approval and explicit application remain required before adoption".to_string(),
        ],
    }
}

fn promotion_recommendation_from_readiness(
    readiness: &SelfEvolutionReadinessReport,
    eval: &DeterministicEvaluation,
    changed: bool,
) -> PromotionRecommendation {
    if eval.failed {
        return PromotionRecommendation {
            recommended: false,
            reason: "baseline-vs-candidate evaluation failed; candidate is not eligible for promotion".to_string(),
            human_approval_required: true,
            promotion_status: "not_promoted_eval_failed".to_string(),
        };
    }
    if !changed {
        return PromotionRecommendation {
            recommended: false,
            reason: "candidate artifact is unchanged from baseline; score deltas would be treated as evaluation noise"
                .to_string(),
            human_approval_required: true,
            promotion_status: "not_promoted".to_string(),
        };
    }
    PromotionRecommendation {
        recommended: true,
        reason: if readiness.label == "promotion_eligible" {
            readiness.reasons.join("; ")
        } else {
            format!(
                "candidate changed and deterministic fake score improved; readiness remains {}: {}",
                readiness.label,
                readiness.reasons.join("; ")
            )
        },
        human_approval_required: true,
        promotion_status: "awaiting_human_approval".to_string(),
    }
}

fn normalized_profile(profile: &str) -> String {
    match profile.trim() {
        "controlled-dogfood" | "controlled_dogfood" => "controlled_dogfood".to_string(),
        "promotion-eligible" | "promotion_eligible" => "promotion_eligible".to_string(),
        "blocked" => "blocked".to_string(),
        _ => "dry_run_only".to_string(),
    }
}

#[derive(Debug, Clone)]
struct DeterministicEvaluation {
    baseline_score: f64,
    candidate_score: f64,
    baseline_status: String,
    candidate_status: String,
    failed: bool,
}

fn deterministic_evaluation(changed: bool, simulate_eval_failure: bool) -> DeterministicEvaluation {
    if simulate_eval_failure {
        return DeterministicEvaluation {
            baseline_score: 0.0,
            candidate_score: 0.0,
            baseline_status: "failed_fake_eval".to_string(),
            candidate_status: "failed_fake_eval".to_string(),
            failed: true,
        };
    }

    DeterministicEvaluation {
        baseline_score: 1.0,
        candidate_score: if changed { 1.1 } else { 1.0 },
        baseline_status: "recorded_not_executed_fake_eval".to_string(),
        candidate_status: "isolated_candidate_written".to_string(),
        failed: false,
    }
}

fn baseline_evidence(failed: bool) -> Vec<String> {
    if failed {
        return vec![
            "baseline command recorded for deterministic dry-run evaluation".to_string(),
            "fake evaluation was configured to fail; no candidate may be promoted from this run".to_string(),
        ];
    }

    vec!["baseline command recorded for deterministic dry-run evaluation".to_string()]
}

fn candidate_evidence(failed: bool) -> Vec<String> {
    if failed {
        return vec![
            "candidate was written under the run-scoped output directory".to_string(),
            "active target artifact was not overwritten".to_string(),
            "candidate evaluation failed in the deterministic fake evaluator".to_string(),
        ];
    }

    vec![
        "candidate was written under the run-scoped output directory".to_string(),
        "active target artifact was not overwritten".to_string(),
    ]
}

fn self_evolution_prompt_len(target: &ArtifactIdentity, baseline_command: &str) -> usize {
    target.path.len() + baseline_command.len()
}
