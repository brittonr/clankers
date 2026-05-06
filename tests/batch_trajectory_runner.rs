use clankers::modes::batch::BatchExecutionMode;
use clankers::modes::batch::BatchJob;
use clankers::modes::batch::BatchJobExecutor;
use clankers::modes::batch::BatchJobStatus;
use clankers::modes::batch::BatchRunConfig;
use clankers::modes::batch::BatchRunManifest;
use clankers::modes::batch::BatchRunSummary;
use clankers::modes::batch::TrajectoryFormat;
use clankers::modes::batch::build_run_manifest;
use clankers::modes::batch::filter_resume_jobs;
use clankers::modes::batch::parse_jsonl_jobs;
use clankers::modes::batch::render_trajectory_results;
use clankers::modes::batch::run_batch_jobs;

struct PrefixExecutor;

#[async_trait::async_trait]
impl BatchJobExecutor for PrefixExecutor {
    async fn execute(&self, job: &BatchJob) -> Result<String, String> {
        Ok(format!("answer: {}", job.prompt))
    }

    fn model_label(&self) -> Option<&str> {
        Some("integration-fake")
    }
}

struct FailingExecutor;

#[async_trait::async_trait]
impl BatchJobExecutor for FailingExecutor {
    async fn execute(&self, job: &BatchJob) -> Result<String, String> {
        Err(format!("failed {} chars", job.prompt.chars().count()))
    }
}

#[tokio::test]
async fn batch_runner_exports_successful_jsonl_trajectory() {
    let jobs = parse_jsonl_jobs(
        r#"{"id":"one","prompt":"first","metadata":{"suite":"integration"}}
{"id":"two","prompt":"second"}"#,
    )
    .expect("valid jobs");
    let config = BatchRunConfig::new("prompts.jsonl", "out/results.jsonl", 2, TrajectoryFormat::Jsonl, false);
    let (summary, results) = run_batch_jobs(&config, jobs, &PrefixExecutor).await.expect("batch succeeds");

    assert_eq!(summary.total, 2);
    assert_eq!(summary.succeeded, 2);
    assert_eq!(summary.failed, 0);
    assert_eq!(results[0].id, "one");
    assert_eq!(results[1].id, "two");
    assert_eq!(results[0].status, BatchJobStatus::Succeeded);
    assert_eq!(results[0].response.as_deref(), Some("answer: first"));

    let rendered = render_trajectory_results(TrajectoryFormat::Jsonl, &results).expect("jsonl renders");
    assert!(rendered.lines().count() == 2);
    assert!(rendered.contains(r#""id":"one""#));
    assert!(!results[0].metadata.as_ref().unwrap().to_string().contains("integration\"}"));
}

#[tokio::test]
async fn batch_runner_records_daemon_session_manifest_and_eval_jsonl() {
    let jobs = parse_jsonl_jobs(
        r#"{"id":"one","prompt":"first","metadata":{"expected_contains":"first"}}
{"id":"two","prompt":"second"}"#,
    )
    .expect("valid jobs");
    let config = BatchRunConfig::new("prompts.jsonl", "out/results.jsonl", 2, TrajectoryFormat::EvalJsonl, false)
        .with_execution(BatchExecutionMode::Daemon)
        .with_run_id(Some("eval-run".to_string()));
    let (summary, results) = run_batch_jobs(&config, jobs, &PrefixExecutor).await.expect("batch succeeds");

    assert_eq!(summary.execution, BatchExecutionMode::Daemon);
    assert_eq!(results[0].session_id.as_deref(), Some("batch-eval-run-one"));
    assert_eq!(results[0].objective.as_ref().unwrap().score, Some(1.0));

    let manifest = build_run_manifest(&summary, &results);
    assert_eq!(manifest.completed_job_ids, vec!["one", "two"]);
    assert_eq!(manifest.session_ids.len(), 2);
    assert!(!serde_json::to_string(&manifest).unwrap().contains("first"));

    let rendered = render_trajectory_results(TrajectoryFormat::EvalJsonl, &results).expect("eval jsonl renders");
    assert!(rendered.contains(r#""run_id":"eval-run""#));
    assert!(rendered.contains(r#""session_id":"batch-eval-run-one""#));
    assert!(rendered.contains(r#""score":1.0"#));
}

#[test]
fn batch_runner_resume_filters_completed_manifest_jobs() {
    let jobs = parse_jsonl_jobs(
        r#"{"id":"done","prompt":"first"}
{"id":"retry","prompt":"second"}"#,
    )
    .expect("valid jobs");
    let summary = BatchRunSummary {
        source: "batch_trajectory_runner".to_string(),
        status: "partial".to_string(),
        run_id: "resume-run".to_string(),
        total: 2,
        succeeded: 1,
        failed: 1,
        skipped: 0,
        concurrency: 2,
        format: TrajectoryFormat::Jsonl,
        execution: BatchExecutionMode::Local,
    };
    let manifest = BatchRunManifest {
        source: "batch_trajectory_runner".to_string(),
        run_id: summary.run_id,
        status: summary.status,
        execution: BatchExecutionMode::Local,
        total: 2,
        succeeded: 1,
        failed: 1,
        skipped: 0,
        completed_job_ids: vec!["done".to_string()],
        failed_job_ids: vec!["retry".to_string()],
        session_ids: Vec::new(),
        redaction: "safe_metadata_only".to_string(),
    };

    let remaining = filter_resume_jobs(jobs, Some(&manifest));
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id.as_deref(), Some("retry"));
}

#[tokio::test]
async fn batch_runner_records_failed_jobs_without_panicking() {
    let jobs = parse_jsonl_jobs(r#"{"id":"bad","prompt":"please fail"}"#).expect("valid jobs");
    let config = BatchRunConfig::new("prompts.jsonl", "out/results.jsonl", 1, TrajectoryFormat::Jsonl, false);
    let (summary, results) = run_batch_jobs(&config, jobs, &FailingExecutor).await.expect("batch completes");

    assert_eq!(summary.status, "partial");
    assert_eq!(summary.succeeded, 0);
    assert_eq!(summary.failed, 1);
    assert_eq!(results[0].status, BatchJobStatus::Failed);
    assert!(results[0].error.as_deref().unwrap_or_default().contains("failed"));
    assert!(results[0].response.is_none());
}

#[test]
fn batch_runner_rejects_unsupported_remote_input() {
    let config = BatchRunConfig::new(
        "https://example.invalid/prompts.jsonl",
        "out/results.jsonl",
        1,
        TrajectoryFormat::Jsonl,
        false,
    );

    let err = config.validate().expect_err("remote input rejected");
    assert!(err.to_string().contains("remote batch input URLs are not supported"));
}
