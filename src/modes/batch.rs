use std::path::Path;
use std::path::PathBuf;

use futures::StreamExt;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;

pub const DEFAULT_BATCH_CONCURRENCY: usize = 4;
pub const MAX_BATCH_CONCURRENCY: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrajectoryFormat {
    Jsonl,
    Sharegpt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchRunConfig {
    pub input: PathBuf,
    pub output: PathBuf,
    pub concurrency: usize,
    pub format: TrajectoryFormat,
    pub resume: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchJob {
    #[serde(default)]
    pub id: Option<String>,
    pub prompt: String,
    #[serde(default)]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchJobStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchJobResult {
    pub id: String,
    pub status: BatchJobStatus,
    pub response: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchRunSummary {
    pub source: &'static str,
    pub status: &'static str,
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub concurrency: usize,
    pub format: TrajectoryFormat,
}

#[async_trait::async_trait]
pub trait BatchJobExecutor: Send + Sync {
    async fn execute(&self, job: &BatchJob) -> Result<String, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchPolicyError {
    BlankPrompt,
    EmptyInput,
    RemoteInputUnsupported,
    RemoteOutputUnsupported,
    ZeroConcurrency,
    ConcurrencyTooHigh { max: usize },
    InvalidMetadata,
    JsonLine { line: usize, message: String },
}

impl std::fmt::Display for BatchPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlankPrompt => write!(f, "batch job prompt must not be blank"),
            Self::EmptyInput => write!(f, "batch input must contain at least one job"),
            Self::RemoteInputUnsupported => write!(f, "remote batch input URLs are not supported in the first pass"),
            Self::RemoteOutputUnsupported => {
                write!(f, "remote batch output destinations are not supported in the first pass")
            }
            Self::ZeroConcurrency => write!(f, "batch concurrency must be greater than zero"),
            Self::ConcurrencyTooHigh { max } => write!(f, "batch concurrency exceeds first-pass maximum of {max}"),
            Self::InvalidMetadata => write!(f, "batch job metadata must be a JSON object when provided"),
            Self::JsonLine { line, message } => write!(f, "invalid batch JSONL at line {line}: {message}"),
        }
    }
}

impl std::error::Error for BatchPolicyError {}

impl BatchRunConfig {
    pub fn new(
        input: impl Into<PathBuf>,
        output: impl Into<PathBuf>,
        concurrency: usize,
        format: TrajectoryFormat,
        resume: bool,
    ) -> Self {
        Self {
            input: input.into(),
            output: output.into(),
            concurrency,
            format,
            resume,
        }
    }

    pub fn validate(&self) -> Result<(), BatchPolicyError> {
        validate_local_path(&self.input, BatchPolicyError::RemoteInputUnsupported)?;
        validate_local_path(&self.output, BatchPolicyError::RemoteOutputUnsupported)?;
        match self.concurrency {
            0 => Err(BatchPolicyError::ZeroConcurrency),
            n if n > MAX_BATCH_CONCURRENCY => Err(BatchPolicyError::ConcurrencyTooHigh {
                max: MAX_BATCH_CONCURRENCY,
            }),
            _ => Ok(()),
        }
    }
}

pub fn parse_jsonl_jobs(input: &str) -> Result<Vec<BatchJob>, BatchPolicyError> {
    let mut jobs = Vec::new();
    for (idx, line) in input.lines().enumerate() {
        let line_number = idx + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let job: BatchJob = serde_json::from_str(trimmed).map_err(|err| BatchPolicyError::JsonLine {
            line: line_number,
            message: err.to_string(),
        })?;
        validate_job(&job).map_err(|err| match err {
            BatchPolicyError::JsonLine { .. } => err,
            other => BatchPolicyError::JsonLine {
                line: line_number,
                message: other.to_string(),
            },
        })?;
        jobs.push(job);
    }
    if jobs.is_empty() {
        return Err(BatchPolicyError::EmptyInput);
    }
    Ok(jobs)
}

pub fn validate_job(job: &BatchJob) -> Result<(), BatchPolicyError> {
    if job.prompt.trim().is_empty() {
        return Err(BatchPolicyError::BlankPrompt);
    }
    if job.metadata.as_ref().is_some_and(|value| !value.is_object()) {
        return Err(BatchPolicyError::InvalidMetadata);
    }
    Ok(())
}

pub async fn run_batch_jobs(
    config: &BatchRunConfig,
    jobs: Vec<BatchJob>,
    executor: &(dyn BatchJobExecutor + Send + Sync),
) -> Result<(BatchRunSummary, Vec<BatchJobResult>), BatchPolicyError> {
    config.validate()?;
    if jobs.is_empty() {
        return Err(BatchPolicyError::EmptyInput);
    }
    for job in &jobs {
        validate_job(job)?;
    }

    let mut indexed = futures::stream::iter(jobs.into_iter().enumerate())
        .map(|(index, job)| async move {
            let id = job.id.clone().unwrap_or_else(|| format!("line-{}", index + 1));
            let metadata = normalized_job_metadata(&job);
            let result = match executor.execute(&job).await {
                Ok(response) => BatchJobResult {
                    id,
                    status: BatchJobStatus::Succeeded,
                    response: Some(response),
                    error: None,
                    metadata: Some(metadata),
                },
                Err(error) => BatchJobResult {
                    id,
                    status: BatchJobStatus::Failed,
                    response: None,
                    error: Some(error),
                    metadata: Some(metadata),
                },
            };
            (index, result)
        })
        .buffer_unordered(config.concurrency)
        .collect::<Vec<_>>()
        .await;
    indexed.sort_by_key(|(index, _)| *index);
    let results: Vec<_> = indexed.into_iter().map(|(_, result)| result).collect();
    let succeeded = results.iter().filter(|result| result.status == BatchJobStatus::Succeeded).count();
    let failed = results.len() - succeeded;
    let summary = BatchRunSummary {
        source: "batch_trajectory_runner",
        status: if failed == 0 { "ok" } else { "partial" },
        total: results.len(),
        succeeded,
        failed,
        concurrency: config.concurrency,
        format: config.format,
    };
    Ok((summary, results))
}

pub fn results_jsonl(results: &[BatchJobResult]) -> Result<String, serde_json::Error> {
    let mut output = String::new();
    for result in results {
        output.push_str(&serde_json::to_string(result)?);
        output.push('\n');
    }
    Ok(output)
}

fn normalized_job_metadata(job: &BatchJob) -> Value {
    json!({
        "source": "batch_trajectory_runner",
        "has_metadata": job.metadata.is_some(),
        "prompt_chars": job.prompt.chars().count(),
    })
}

fn validate_local_path(path: &Path, err: BatchPolicyError) -> Result<(), BatchPolicyError> {
    let rendered = path.to_string_lossy();
    if rendered.starts_with("http://") || rendered.starts_with("https://") || rendered.starts_with("s3://") {
        return Err(err);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_batch_jsonl_jobs() {
        let jobs = parse_jsonl_jobs(
            r#"{"id":"a","prompt":"one","metadata":{"suite":"smoke"}}
{"prompt":"two"}"#,
        )
        .expect("jobs parse");

        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].id.as_deref(), Some("a"));
        assert_eq!(jobs[0].metadata.as_ref().unwrap()["suite"], "smoke");
        assert_eq!(jobs[1].prompt, "two");
    }

    #[test]
    fn rejects_blank_prompt_with_line_number() {
        let err = parse_jsonl_jobs(r#"{"id":"bad","prompt":"   "}"#).expect_err("blank prompt rejected");
        assert!(err.to_string().contains("line 1"));
        assert!(err.to_string().contains("prompt must not be blank"));
    }

    #[test]
    fn rejects_non_object_metadata() {
        let err = parse_jsonl_jobs(r#"{"prompt":"x","metadata":["leak"]}"#).expect_err("metadata rejected");
        assert!(err.to_string().contains("metadata must be a JSON object"));
    }

    #[test]
    fn validates_local_bounded_batch_config() {
        let cfg =
            BatchRunConfig::new("prompts.jsonl", "out", DEFAULT_BATCH_CONCURRENCY, TrajectoryFormat::Sharegpt, true);
        cfg.validate().expect("local bounded config valid");
    }

    #[test]
    fn rejects_remote_input_and_unbounded_concurrency() {
        let remote = BatchRunConfig::new(
            "https://example.invalid/prompts.jsonl",
            "out",
            DEFAULT_BATCH_CONCURRENCY,
            TrajectoryFormat::Jsonl,
            false,
        );
        assert_eq!(remote.validate().unwrap_err(), BatchPolicyError::RemoteInputUnsupported);

        let too_many =
            BatchRunConfig::new("prompts.jsonl", "out", MAX_BATCH_CONCURRENCY + 1, TrajectoryFormat::Jsonl, false);
        assert_eq!(too_many.validate().unwrap_err(), BatchPolicyError::ConcurrencyTooHigh {
            max: MAX_BATCH_CONCURRENCY
        });
    }

    struct EchoExecutor;

    #[async_trait::async_trait]
    impl BatchJobExecutor for EchoExecutor {
        async fn execute(&self, job: &BatchJob) -> Result<String, String> {
            Ok(format!("echo: {}", job.prompt))
        }
    }

    #[tokio::test]
    async fn batch_backend_runs_jobs_and_redacts_prompt_metadata() {
        let cfg = BatchRunConfig::new("prompts.jsonl", "out", 2, TrajectoryFormat::Jsonl, false);
        let jobs = parse_jsonl_jobs(
            r#"{"id":"a","prompt":"secret prompt"}
{"prompt":"second"}"#,
        )
        .unwrap();
        let (summary, results) = run_batch_jobs(&cfg, jobs, &EchoExecutor).await.unwrap();

        assert_eq!(summary.source, "batch_trajectory_runner");
        assert_eq!(summary.total, 2);
        assert_eq!(summary.succeeded, 2);
        assert_eq!(results[0].id, "a");
        assert_eq!(results[1].id, "line-2");
        assert!(!results[0].metadata.as_ref().unwrap().to_string().contains("secret prompt"));
    }

    #[test]
    fn renders_result_jsonl() {
        let result = BatchJobResult {
            id: "a".to_string(),
            status: BatchJobStatus::Succeeded,
            response: Some("ok".to_string()),
            error: None,
            metadata: None,
        };
        let rendered = results_jsonl(&[result]).unwrap();
        assert!(rendered.contains(r#""id":"a""#));
        assert!(rendered.ends_with('\n'));
    }
}
