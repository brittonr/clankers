//! Disabled-by-default self-evolution dry-run model.
//!
//! This module intentionally models self-evolution as an offline orchestration
//! run. The first implementation writes only run-scoped artifacts, uses a fake
//! MCP/session-control executor for deterministic tests, and never promotes or
//! mutates active artifacts.

use serde_json::Value;
use serde_json::json;
mod application;
mod approval;
mod receipts;
mod rollback;
mod run;
mod validation;

pub use application::apply_self_evolution_candidate;
pub use application::preflight_self_evolution_application;
pub use approval::approve_self_evolution_promotion;
pub use receipts::*;
pub use rollback::rollback_self_evolution_application;
pub use run::load_eval_corpus_manifest;
pub use run::run_self_evolution_dry_run;
pub use validation::application_receipt_path;
pub use validation::approval_receipt_path;
#[cfg(test)]
use validation::read_run_receipt;
pub use validation::rollback_receipt_path;
pub use validation::validate_application_options;
#[cfg(test)]
use validation::validate_approval_options;
pub use validation::validate_run_options;

pub trait SelfEvolutionExecutor {
    fn submit_tool(&mut self, session_id: Option<&str>, tool: &str, arguments: Value) -> McpOrchestrationReceipt;
}

#[derive(Debug, Default)]
pub struct FakeMcpExecutor {
    pub calls: Vec<McpOrchestrationReceipt>,
}

impl SelfEvolutionExecutor for FakeMcpExecutor {
    fn submit_tool(&mut self, session_id: Option<&str>, tool: &str, arguments: Value) -> McpOrchestrationReceipt {
        let receipt = McpOrchestrationReceipt {
            source: "mcp_session_control_fake_executor".to_string(),
            session_id: session_id.map(ToString::to_string),
            tool: tool.to_string(),
            status: "submitted_to_fake_session_control".to_string(),
            submitted: true,
            request_summary: summarize_mcp_arguments(tool, &arguments),
        };
        self.calls.push(receipt.clone());
        receipt
    }
}

fn summarize_mcp_arguments(tool: &str, arguments: &Value) -> Value {
    match tool {
        "send_prompt" => json!({
            "tool": tool,
            "prompt_len": arguments.get("prompt_len").and_then(Value::as_u64).unwrap_or(0),
            "purpose": arguments.get("purpose").and_then(Value::as_str).unwrap_or("unknown"),
        }),
        _ => json!({
            "tool": tool,
            "argument_keys": arguments.as_object().map(|object| object.len()).unwrap_or(0),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    use chrono::SecondsFormat;
    use chrono::Utc;
    use tempfile::TempDir;
    use tempfile::tempdir;

    use super::*;

    fn write_valid_corpus_manifest(tmp: &TempDir) -> PathBuf {
        let manifest_path = tmp.path().join("corpus.json");
        fs::write(
            &manifest_path,
            serde_json::json!({
                "version": 1,
                "targets": ["prompt.md"],
                "cases": [{
                    "id": "case-1",
                    "objective": "candidate improves deterministic score",
                    "oracle_command": "cargo test self_evolution",
                    "expected_evidence": ["receipt.json", "session_history"]
                }],
                "redaction_policy": "safe metadata only",
                "min_improvement": 0.05,
                "regression_budget": 0
            })
            .to_string(),
        )
        .unwrap();
        manifest_path
    }

    #[test]
    fn self_evolution_dry_run_writes_isolated_receipt_and_uses_fake_mcp() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("skill.md");
        fs::write(&target, "baseline skill\n").unwrap();
        let output = tmp.path().join("candidates");
        let options = SelfEvolutionRunOptions {
            target: target.clone(),
            baseline_command: "cargo test fake_eval".to_string(),
            candidate_output: output,
            session_id: Some("sess-1".to_string()),
            dry_run: true,
            simulate_eval_failure: false,
            candidate_body: Some("baseline skill\nimproved\n".to_string()),
            production_profile: "dry-run-only".to_string(),
            corpus_manifest: None,
        };
        let mut executor = FakeMcpExecutor::default();

        let receipt = run_self_evolution_dry_run(&options, &mut executor).unwrap();

        assert_eq!(receipt.source, "self_evolution_dry_run");
        assert_eq!(receipt.status, "completed");
        assert!(receipt.candidate.changed_from_baseline);
        assert!(receipt.recommendation.recommended);
        assert!(receipt.recommendation.human_approval_required);
        assert_eq!(receipt.readiness.label, "dry_run_only");
        assert_eq!(receipt.readiness.corpus_cases, 0);
        assert_eq!(executor.calls.len(), 2);
        assert_eq!(executor.calls[0].tool, "send_prompt");
        assert_eq!(executor.calls[1].tool, "session_history");
        assert!(Path::new(&receipt.candidate.artifact_path).exists());
        assert!(Path::new(&receipt.candidate.output_dir).join("receipt.json").exists());
        assert_eq!(fs::read_to_string(&target).unwrap(), "baseline skill\n");
    }

    #[test]
    fn self_evolution_unchanged_candidate_is_not_recommended_as_noise() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("prompt.md");
        fs::write(&target, "same\n").unwrap();
        let options = SelfEvolutionRunOptions {
            target,
            baseline_command: "eval".to_string(),
            candidate_output: tmp.path().join("out"),
            session_id: None,
            dry_run: true,
            simulate_eval_failure: false,
            candidate_body: None,
            production_profile: "dry-run-only".to_string(),
            corpus_manifest: None,
        };
        let mut executor = FakeMcpExecutor::default();

        let receipt = run_self_evolution_dry_run(&options, &mut executor).unwrap();

        assert!(!receipt.candidate.changed_from_baseline);
        assert!(!receipt.recommendation.recommended);
        assert!(receipt.recommendation.reason.contains("evaluation noise"));
        let receipt_path = Path::new(&receipt.candidate.output_dir).join("receipt.json");
        let saved: SelfEvolutionRunReceipt = serde_json::from_str(&fs::read_to_string(receipt_path).unwrap()).unwrap();
        assert!(!saved.recommendation.recommended);
        assert_eq!(saved.recommendation.promotion_status, "not_promoted");
        assert!(saved.recommendation.human_approval_required);
    }

    #[test]
    fn self_evolution_failed_eval_records_non_promotable_receipt() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("prompt.md");
        fs::write(&target, "baseline\n").unwrap();
        let options = SelfEvolutionRunOptions {
            target,
            baseline_command: "eval --fixture failure".to_string(),
            candidate_output: tmp.path().join("out"),
            session_id: Some("sess-1".to_string()),
            dry_run: true,
            simulate_eval_failure: true,
            candidate_body: Some("candidate\n".to_string()),
            production_profile: "promotion-eligible".to_string(),
            corpus_manifest: Some(write_valid_corpus_manifest(&tmp)),
        };
        let mut executor = FakeMcpExecutor::default();

        let receipt = run_self_evolution_dry_run(&options, &mut executor).unwrap();

        assert_eq!(receipt.status, "completed_with_failed_evaluation");
        assert_eq!(receipt.baseline.status, "failed_fake_eval");
        assert_eq!(receipt.candidate.status, "failed_fake_eval");
        assert!(receipt.candidate.changed_from_baseline);
        assert!(!receipt.recommendation.recommended);
        assert_eq!(receipt.recommendation.promotion_status, "not_promoted_eval_failed");
        assert!(receipt.recommendation.reason.contains("evaluation failed"));
        assert!(receipt.baseline.evidence.iter().any(|entry| entry.contains("fail")));
        assert_eq!(executor.calls.len(), 2);
    }

    #[test]
    fn self_evolution_production_profile_requires_valid_corpus_manifest() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("prompt.md");
        fs::write(&target, "baseline\n").unwrap();
        let options = SelfEvolutionRunOptions {
            target,
            baseline_command: "eval".to_string(),
            candidate_output: tmp.path().join("out"),
            session_id: Some("sess-1".to_string()),
            dry_run: true,
            simulate_eval_failure: false,
            candidate_body: Some("candidate\n".to_string()),
            production_profile: "controlled-dogfood".to_string(),
            corpus_manifest: None,
        };
        let mut executor = FakeMcpExecutor::default();

        let receipt = run_self_evolution_dry_run(&options, &mut executor).unwrap();

        assert!(receipt.recommendation.recommended);
        assert_eq!(receipt.recommendation.promotion_status, "awaiting_human_approval");
        assert_eq!(receipt.readiness.label, "blocked");
        assert!(receipt.readiness.reasons.iter().any(|reason| reason.contains("corpus manifest")));
        assert!(receipt.readiness.daemon_session_observable);
    }

    #[test]
    fn self_evolution_rejects_invalid_corpus_manifest() {
        let tmp = tempdir().unwrap();
        let manifest_path = tmp.path().join("bad-corpus.json");
        fs::write(
            &manifest_path,
            serde_json::json!({
                "version": 1,
                "targets": [],
                "cases": [],
                "redaction_policy": "safe metadata only",
                "min_improvement": 0.05,
                "regression_budget": 0
            })
            .to_string(),
        )
        .unwrap();

        let err = load_eval_corpus_manifest(Some(&manifest_path)).unwrap_err();

        assert!(err.contains("at least one target"));
    }

    #[test]
    fn self_evolution_valid_corpus_can_mark_promotion_eligible_readiness() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("prompt.md");
        fs::write(&target, "baseline\n").unwrap();
        let manifest_path = tmp.path().join("corpus.json");
        fs::write(
            &manifest_path,
            serde_json::json!({
                "version": 1,
                "targets": ["prompt.md"],
                "cases": [{
                    "id": "case-1",
                    "objective": "candidate improves deterministic score",
                    "oracle_command": "cargo test self_evolution",
                    "expected_evidence": ["receipt.json", "session_history"]
                }],
                "redaction_policy": "safe metadata only",
                "min_improvement": 0.05,
                "regression_budget": 0
            })
            .to_string(),
        )
        .unwrap();
        let options = SelfEvolutionRunOptions {
            target,
            baseline_command: "eval".to_string(),
            candidate_output: tmp.path().join("out"),
            session_id: Some("sess-1".to_string()),
            dry_run: true,
            simulate_eval_failure: false,
            candidate_body: Some("candidate\n".to_string()),
            production_profile: "promotion-eligible".to_string(),
            corpus_manifest: Some(manifest_path.clone()),
        };
        let mut executor = FakeMcpExecutor::default();

        let receipt = run_self_evolution_dry_run(&options, &mut executor).unwrap();

        assert_eq!(receipt.readiness.label, "promotion_eligible");
        assert_eq!(receipt.readiness.corpus_manifest_path, Some(manifest_path.display().to_string()));
        assert_eq!(receipt.readiness.corpus_cases, 1);
        assert!(receipt.readiness.threshold_passed);
        assert!(receipt.readiness.regression_budget_passed);
        assert!(receipt.readiness.unchanged_candidate_control_passed);
        assert!(receipt.readiness.daemon_session_observable);
        assert_eq!(receipt.recommendation.promotion_status, "awaiting_human_approval");
    }

    #[test]
    fn self_evolution_rejects_live_in_place_and_non_dry_run() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("live");
        fs::create_dir_all(&target).unwrap();
        let non_dry = SelfEvolutionRunOptions {
            target: target.clone(),
            baseline_command: "eval".to_string(),
            candidate_output: tmp.path().join("out"),
            session_id: None,
            dry_run: false,
            simulate_eval_failure: false,
            candidate_body: None,
            production_profile: "dry-run-only".to_string(),
            corpus_manifest: None,
        };
        assert!(validate_run_options(&non_dry).unwrap_err().contains("disabled by default"));

        let in_place = SelfEvolutionRunOptions {
            target: target.clone(),
            baseline_command: "eval".to_string(),
            candidate_output: target.join("candidate"),
            session_id: None,
            dry_run: true,
            simulate_eval_failure: false,
            candidate_body: None,
            production_profile: "dry-run-only".to_string(),
            corpus_manifest: None,
        };
        assert!(validate_run_options(&in_place).unwrap_err().contains("live target directory"));
    }

    #[test]
    fn self_evolution_approval_records_confirmation_receipt_without_applying() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("tool.md");
        fs::write(&target, "baseline\n").unwrap();
        let options = SelfEvolutionRunOptions {
            target: target.clone(),
            baseline_command: "eval".to_string(),
            candidate_output: tmp.path().join("out"),
            session_id: Some("sess-1".to_string()),
            dry_run: true,
            simulate_eval_failure: false,
            candidate_body: Some("candidate\n".to_string()),
            production_profile: "promotion-eligible".to_string(),
            corpus_manifest: Some(write_valid_corpus_manifest(&tmp)),
        };
        let mut executor = FakeMcpExecutor::default();
        let run_receipt = run_self_evolution_dry_run(&options, &mut executor).unwrap();
        let run_receipt_path = Path::new(&run_receipt.candidate.output_dir).join("receipt.json");
        let approval_options = SelfEvolutionApprovalOptions {
            receipt_path: run_receipt_path.clone(),
            session_id: "sess-1".to_string(),
            confirmation_id: "confirm-1".to_string(),
            approver: "human-reviewer".to_string(),
            dry_run: true,
        };
        let mut approval_executor = FakeMcpExecutor::default();

        let approval = approve_self_evolution_promotion(&approval_options, &mut approval_executor).unwrap();

        assert_eq!(approval.source, "self_evolution_promotion_gate");
        assert_eq!(approval.status, "approval_recorded");
        assert!(approval.approval.approved);
        assert!(!approval.approval.applied);
        assert_eq!(approval.approval.promotion_status, "approval_recorded_not_applied");
        assert_eq!(approval_executor.calls.len(), 2);
        assert_eq!(approval_executor.calls[0].tool, "approve_confirmation");
        assert_eq!(approval_executor.calls[1].tool, "session_history");
        assert!(approval_receipt_path(&run_receipt_path).exists());
        assert_eq!(fs::read_to_string(&target).unwrap(), "baseline\n");
    }

    #[test]
    fn self_evolution_approval_rejects_unrecommended_or_ungated_candidates() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("prompt.md");
        fs::write(&target, "same\n").unwrap();
        let options = SelfEvolutionRunOptions {
            target,
            baseline_command: "eval".to_string(),
            candidate_output: tmp.path().join("out"),
            session_id: Some("sess-1".to_string()),
            dry_run: true,
            simulate_eval_failure: false,
            candidate_body: None,
            production_profile: "dry-run-only".to_string(),
            corpus_manifest: None,
        };
        let mut executor = FakeMcpExecutor::default();
        let run_receipt = run_self_evolution_dry_run(&options, &mut executor).unwrap();
        let run_receipt_path = Path::new(&run_receipt.candidate.output_dir).join("receipt.json");
        let approval_options = SelfEvolutionApprovalOptions {
            receipt_path: run_receipt_path,
            session_id: "sess-1".to_string(),
            confirmation_id: "confirm-1".to_string(),
            approver: "reviewer".to_string(),
            dry_run: true,
        };
        let mut approval_executor = FakeMcpExecutor::default();

        let err = approve_self_evolution_promotion(&approval_options, &mut approval_executor).unwrap_err();

        assert!(err.contains("not recommended"));
        assert!(approval_executor.calls.is_empty());
    }

    #[test]
    fn self_evolution_approval_requires_dry_run_and_confirmation_context() {
        let options = SelfEvolutionApprovalOptions {
            receipt_path: PathBuf::from("receipt.json"),
            session_id: String::new(),
            confirmation_id: "confirm-1".to_string(),
            approver: "reviewer".to_string(),
            dry_run: true,
        };
        assert!(validate_approval_options(&options).unwrap_err().contains("session id"));

        let non_dry = SelfEvolutionApprovalOptions {
            dry_run: false,
            session_id: "sess-1".to_string(),
            ..options
        };
        assert!(validate_approval_options(&non_dry).unwrap_err().contains("disabled by default"));
    }

    #[test]
    fn self_evolution_application_preflight_validates_without_mutation() {
        let (_tmp, target, run_receipt_path, approval_path) = approved_candidate_fixture();
        let original_target = fs::read_to_string(&target).unwrap();
        let options = SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path.clone(),
            approval_path: approval_path.clone(),
            apply_mode: "replace-file".to_string(),
            verification_command: "cargo test self_evolution".to_string(),
            dry_run: true,
        };

        let receipt = preflight_self_evolution_application(&options).unwrap();

        assert_eq!(receipt.source, "self_evolution_candidate_application");
        assert_eq!(receipt.status, "preflight_validated");
        assert!(receipt.dry_run);
        assert!(!receipt.applied);
        assert_eq!(receipt.apply_mode, "replace-file");
        assert_eq!(receipt.verification.status, "recorded_not_executed_dry_run");
        assert_eq!(fs::read_to_string(&target).unwrap(), original_target);
        assert!(!Path::new(&receipt.planned_backup_path).exists());
        assert!(!application_receipt_path(&run_receipt_path).exists());
        assert_eq!(receipt.approval_receipt_path, approval_path.display().to_string());
    }

    #[test]
    fn self_evolution_application_rejects_stale_target_before_mutation() {
        let (_tmp, target, run_receipt_path, approval_path) = approved_candidate_fixture();
        fs::write(&target, "changed after receipt\n").unwrap();
        let options = SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path,
            approval_path,
            apply_mode: "replace-file".to_string(),
            verification_command: "cargo test self_evolution".to_string(),
            dry_run: true,
        };

        let err = preflight_self_evolution_application(&options).unwrap_err();

        assert!(err.contains("target artifact changed"));
        assert_eq!(fs::read_to_string(&target).unwrap(), "changed after receipt\n");
    }

    #[test]
    fn self_evolution_application_rejects_mismatched_or_applied_approval() {
        let (_tmp, target, run_receipt_path, approval_path) = approved_candidate_fixture();
        let mut approval: SelfEvolutionApprovalReceipt =
            serde_json::from_str(&fs::read_to_string(&approval_path).unwrap()).unwrap();
        approval.candidate_path = target.display().to_string();
        fs::write(&approval_path, serde_json::to_string_pretty(&approval).unwrap()).unwrap();
        let options = SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path.clone(),
            approval_path: approval_path.clone(),
            apply_mode: "replace-file".to_string(),
            verification_command: "cargo test self_evolution".to_string(),
            dry_run: true,
        };

        let err = preflight_self_evolution_application(&options).unwrap_err();

        assert!(err.contains("candidate path does not match"));
        approval.candidate_path = read_run_receipt(&run_receipt_path).unwrap().candidate.artifact_path;
        approval.approval.applied = true;
        fs::write(&approval_path, serde_json::to_string_pretty(&approval).unwrap()).unwrap();
        let err = preflight_self_evolution_application(&options).unwrap_err();
        assert!(err.contains("already marked applied"));
    }

    #[test]
    fn self_evolution_application_rejects_non_promotable_missing_candidate_and_unsupported_mode() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("prompt.md");
        fs::write(&target, "same\n").unwrap();
        let options = SelfEvolutionRunOptions {
            target: target.clone(),
            baseline_command: "eval".to_string(),
            candidate_output: tmp.path().join("out"),
            session_id: Some("sess-1".to_string()),
            dry_run: true,
            simulate_eval_failure: false,
            candidate_body: None,
            production_profile: "dry-run-only".to_string(),
            corpus_manifest: None,
        };
        let mut executor = FakeMcpExecutor::default();
        let run_receipt = run_self_evolution_dry_run(&options, &mut executor).unwrap();
        let run_receipt_path = Path::new(&run_receipt.candidate.output_dir).join("receipt.json");
        let approval = SelfEvolutionApprovalReceipt {
            source: "test".to_string(),
            run_id: run_receipt.run_id.clone(),
            status: "approval_recorded".to_string(),
            dry_run: true,
            approver: "reviewer".to_string(),
            confirmation_id: "confirm-1".to_string(),
            target_path: run_receipt.target.path.clone(),
            candidate_path: run_receipt.candidate.artifact_path.clone(),
            approval: PromotionApprovalRecord {
                approved: true,
                human_approval_required: true,
                applied: false,
                promotion_status: "approval_recorded_not_applied".to_string(),
                evidence: Vec::new(),
            },
            mcp_receipts: Vec::new(),
            created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        };
        let approval_path = approval_receipt_path(&run_receipt_path);
        fs::write(&approval_path, serde_json::to_string_pretty(&approval).unwrap()).unwrap();
        let preflight = SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path.clone(),
            approval_path: approval_path.clone(),
            apply_mode: "replace-file".to_string(),
            verification_command: "cargo test self_evolution".to_string(),
            dry_run: true,
        };
        let err = preflight_self_evolution_application(&preflight).unwrap_err();
        assert!(err.contains("not eligible"));

        let (_tmp, _target, run_receipt_path, approval_path) = approved_candidate_fixture();
        let run_receipt = read_run_receipt(&run_receipt_path).unwrap();
        fs::remove_file(&run_receipt.candidate.artifact_path).unwrap();
        let err = preflight_self_evolution_application(&SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path,
            approval_path,
            apply_mode: "replace-file".to_string(),
            verification_command: "cargo test self_evolution".to_string(),
            dry_run: true,
        })
        .unwrap_err();
        assert!(err.contains("candidate artifact"));

        let unsupported = SelfEvolutionApplicationOptions {
            receipt_path: PathBuf::from("receipt.json"),
            approval_path: PathBuf::from("approval.json"),
            apply_mode: "patch".to_string(),
            verification_command: "cargo test self_evolution".to_string(),
            dry_run: true,
        };
        assert!(validate_application_options(&unsupported).unwrap_err().contains("unsupported"));
    }

    #[test]
    fn self_evolution_application_live_replace_writes_backup_receipt_and_target() {
        let (_tmp, target, run_receipt_path, approval_path) = approved_candidate_fixture();
        let options = SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path.clone(),
            approval_path,
            apply_mode: "replace-file".to_string(),
            verification_command: "true".to_string(),
            dry_run: false,
        };

        let receipt = apply_self_evolution_candidate(&options).unwrap();

        assert_eq!(receipt.status, "applied");
        assert!(receipt.applied);
        assert!(!receipt.dry_run);
        assert_eq!(receipt.verification.status, "passed");
        assert_eq!(fs::read_to_string(&target).unwrap(), "candidate\n");
        assert_eq!(fs::read_to_string(&receipt.planned_backup_path).unwrap(), "baseline\n");
        assert!(application_receipt_path(&run_receipt_path).exists());
        let saved: SelfEvolutionApplicationReceipt =
            serde_json::from_str(&fs::read_to_string(application_receipt_path(&run_receipt_path)).unwrap()).unwrap();
        assert_eq!(saved.status, "applied");
        assert_eq!(saved.backup_sha256, Some(receipt.pre_apply_sha256));
        assert_eq!(saved.post_apply_sha256, Some(receipt.candidate_sha256));
    }

    #[test]
    fn self_evolution_application_records_verification_failure_after_live_replace() {
        let (_tmp, target, run_receipt_path, approval_path) = approved_candidate_fixture();
        let options = SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path.clone(),
            approval_path,
            apply_mode: "replace-file".to_string(),
            verification_command: "false".to_string(),
            dry_run: false,
        };

        let receipt = apply_self_evolution_candidate(&options).unwrap();

        assert_eq!(receipt.status, "applied_verification_failed");
        assert!(receipt.applied);
        assert_eq!(receipt.verification.status, "failed");
        assert_eq!(fs::read_to_string(&target).unwrap(), "candidate\n");
        assert_eq!(fs::read_to_string(&receipt.planned_backup_path).unwrap(), "baseline\n");
        assert!(application_receipt_path(&run_receipt_path).exists());
        assert!(receipt.rollback.instructions.iter().any(|step| step.contains("restore")));
    }

    #[test]
    fn self_evolution_rollback_preflights_and_restores_backup_with_hash_guard() {
        let (_tmp, target, run_receipt_path, approval_path) = approved_candidate_fixture();
        let apply = apply_self_evolution_candidate(&SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path.clone(),
            approval_path,
            apply_mode: "replace-file".to_string(),
            verification_command: "true".to_string(),
            dry_run: false,
        })
        .unwrap();
        let application_path = application_receipt_path(&run_receipt_path);

        let preflight = rollback_self_evolution_application(&SelfEvolutionRollbackOptions {
            application_path: application_path.clone(),
            dry_run: true,
        })
        .unwrap();
        assert_eq!(preflight.status, "rollback_preflight_validated");
        assert!(!preflight.restored);
        assert_eq!(fs::read_to_string(&target).unwrap(), "candidate\n");
        assert!(!rollback_receipt_path(&application_path).exists());

        let rollback = rollback_self_evolution_application(&SelfEvolutionRollbackOptions {
            application_path: application_path.clone(),
            dry_run: false,
        })
        .unwrap();
        assert_eq!(rollback.status, "rolled_back");
        assert!(rollback.restored);
        assert_eq!(rollback.backup_sha256, apply.pre_apply_sha256);
        assert_eq!(fs::read_to_string(&target).unwrap(), "baseline\n");
        assert!(rollback_receipt_path(&application_path).exists());
    }

    #[test]
    fn self_evolution_rollback_rejects_target_changed_after_application() {
        let (_tmp, target, run_receipt_path, approval_path) = approved_candidate_fixture();
        apply_self_evolution_candidate(&SelfEvolutionApplicationOptions {
            receipt_path: run_receipt_path.clone(),
            approval_path,
            apply_mode: "replace-file".to_string(),
            verification_command: "true".to_string(),
            dry_run: false,
        })
        .unwrap();
        let application_path = application_receipt_path(&run_receipt_path);
        fs::write(&target, "operator changed applied target\n").unwrap();

        let err = rollback_self_evolution_application(&SelfEvolutionRollbackOptions {
            application_path,
            dry_run: false,
        })
        .unwrap_err();

        assert!(err.contains("target artifact changed since application"));
        assert_eq!(fs::read_to_string(&target).unwrap(), "operator changed applied target\n");
    }

    fn approved_candidate_fixture() -> (TempDir, PathBuf, PathBuf, PathBuf) {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("tool.md");
        fs::write(&target, "baseline\n").unwrap();
        let options = SelfEvolutionRunOptions {
            target: target.clone(),
            baseline_command: "eval".to_string(),
            candidate_output: tmp.path().join("out"),
            session_id: Some("sess-1".to_string()),
            dry_run: true,
            simulate_eval_failure: false,
            candidate_body: Some("candidate\n".to_string()),
            production_profile: "promotion-eligible".to_string(),
            corpus_manifest: Some(write_valid_corpus_manifest(&tmp)),
        };
        let mut executor = FakeMcpExecutor::default();
        let run_receipt = run_self_evolution_dry_run(&options, &mut executor).unwrap();
        let run_receipt_path = Path::new(&run_receipt.candidate.output_dir).join("receipt.json");
        let approval_options = SelfEvolutionApprovalOptions {
            receipt_path: run_receipt_path.clone(),
            session_id: "sess-1".to_string(),
            confirmation_id: "confirm-1".to_string(),
            approver: "reviewer".to_string(),
            dry_run: true,
        };
        let mut approval_executor = FakeMcpExecutor::default();
        approve_self_evolution_promotion(&approval_options, &mut approval_executor).unwrap();
        let approval_path = approval_receipt_path(&run_receipt_path);
        (tmp, target, run_receipt_path, approval_path)
    }
}
