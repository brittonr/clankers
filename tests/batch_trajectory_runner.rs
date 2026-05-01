use clankers::modes::batch::BatchJob;
use clankers::modes::batch::BatchJobExecutor;
use clankers::modes::batch::BatchJobStatus;
use clankers::modes::batch::BatchRunConfig;
use clankers::modes::batch::TrajectoryFormat;
use clankers::modes::batch::parse_jsonl_jobs;
use clankers::modes::batch::render_trajectory_results;
use clankers::modes::batch::run_batch_jobs;

struct PrefixExecutor;

#[async_trait::async_trait]
impl BatchJobExecutor for PrefixExecutor {
    async fn execute(&self, job: &BatchJob) -> Result<String, String> {
        Ok(format!("answer: {}", job.prompt))
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
    assert!(!rendered.contains("integration\"}"));
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
