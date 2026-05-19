use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;

use futures::StreamExt;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use sha2::Digest;
use sha2::Sha256;

pub const DEFAULT_BATCH_CONCURRENCY: usize = 4;
pub const MAX_BATCH_CONCURRENCY: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrajectoryFormat {
    Jsonl,
    Sharegpt,
    EvalJsonl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchExecutionMode {
    Local,
    Daemon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchRunConfig {
    pub input: PathBuf,
    pub output: PathBuf,
    pub concurrency: usize,
    pub format: TrajectoryFormat,
    pub resume: bool,
    pub execution: BatchExecutionMode,
    pub run_id: Option<String>,
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
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchJobResult {
    pub id: String,
    pub status: BatchJobStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    pub response: Option<String>,
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective: Option<ObjectiveReceipt>,
    #[serde(default)]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectiveReceipt {
    pub status: String,
    pub score: Option<f64>,
    pub metric: String,
    pub redaction: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchRunSummary {
    pub source: String,
    pub status: String,
    pub run_id: String,
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub concurrency: usize,
    pub format: TrajectoryFormat,
    pub execution: BatchExecutionMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchRunManifest {
    pub source: String,
    pub run_id: String,
    pub status: String,
    pub execution: BatchExecutionMode,
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub completed_job_ids: Vec<String>,
    pub failed_job_ids: Vec<String>,
    pub session_ids: Vec<String>,
    pub redaction: String,
}

#[async_trait::async_trait]
pub trait BatchJobExecutor: Send + Sync {
    async fn execute(&self, job: &BatchJob) -> Result<String, String>;

    async fn execute_batch_job(
        &self,
        _run_id: &str,
        _job_id: &str,
        _session_id: Option<&str>,
        job: &BatchJob,
    ) -> Result<String, String> {
        self.execute(job).await
    }

    fn model_label(&self) -> Option<&str> {
        None
    }

    fn session_id_for(&self, _run_id: &str, _job_id: &str, _job: &BatchJob) -> Option<String> {
        None
    }
}

/// Batch executor that runs existing batch/headless prompts through the embeddable runtime facade.
pub struct RuntimeFacadeBatchExecutor;

#[async_trait::async_trait]
impl BatchJobExecutor for RuntimeFacadeBatchExecutor {
    async fn execute(&self, job: &BatchJob) -> Result<String, String> {
        self.execute_batch_job("batch", "job", None, job).await
    }

    async fn execute_batch_job(
        &self,
        _run_id: &str,
        _job_id: &str,
        session_id: Option<&str>,
        job: &BatchJob,
    ) -> Result<String, String> {
        let runtime = clankers_runtime::RuntimeBuilder::new().build().map_err(|err| err.safe_message())?;
        let session = runtime
            .create_session(clankers_runtime::SessionOptions {
                session_id: session_id.map(clankers_runtime::SessionId::from_host),
                model: self.model_label().map(str::to_string),
            })
            .await
            .map_err(|err| err.safe_message())?;
        let mut events = session.take_events().await.map_err(|err| err.safe_message())?;
        session
            .submit_prompt(clankers_runtime::PromptInput::new(job.prompt.clone()))
            .await
            .map_err(|err| err.safe_message())?;

        let mut assistant = String::new();
        while let Some(event) = events.recv().await {
            match event {
                clankers_runtime::SessionEvent::AssistantDelta { text, .. } => assistant.push_str(&text),
                clankers_runtime::SessionEvent::Completed { .. } => break,
                clankers_runtime::SessionEvent::Error { message, .. } => return Err(message),
                _ => {}
            }
        }
        Ok(assistant)
    }

    fn model_label(&self) -> Option<&str> {
        Some("runtime-echo")
    }
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
    InvalidRunId,
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
            Self::InvalidRunId => write!(f, "batch run id must contain only ASCII letters, digits, '.', '_', or '-'"),
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
            execution: BatchExecutionMode::Local,
            run_id: None,
        }
    }

    pub fn with_execution(mut self, execution: BatchExecutionMode) -> Self {
        self.execution = execution;
        self
    }

    pub fn with_run_id(mut self, run_id: Option<String>) -> Self {
        self.run_id = run_id;
        self
    }

    pub fn validate(&self) -> Result<(), BatchPolicyError> {
        validate_local_path(&self.input, BatchPolicyError::RemoteInputUnsupported)?;
        validate_local_path(&self.output, BatchPolicyError::RemoteOutputUnsupported)?;
        if self.run_id.as_ref().is_some_and(|run_id| !is_safe_id(run_id)) {
            return Err(BatchPolicyError::InvalidRunId);
        }
        match self.concurrency {
            0 => Err(BatchPolicyError::ZeroConcurrency),
            n if n > MAX_BATCH_CONCURRENCY => Err(BatchPolicyError::ConcurrencyTooHigh {
                max: MAX_BATCH_CONCURRENCY,
            }),
            _ => Ok(()),
        }
    }

    pub fn effective_run_id(&self) -> String {
        self.run_id.clone().unwrap_or_else(|| stable_run_id(&self.input, &self.output))
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

pub fn filter_resume_jobs(jobs: Vec<BatchJob>, manifest: Option<&BatchRunManifest>) -> Vec<BatchJob> {
    let Some(manifest) = manifest else {
        return jobs;
    };
    let completed: BTreeSet<&str> = manifest.completed_job_ids.iter().map(String::as_str).collect();
    jobs.into_iter()
        .enumerate()
        .filter_map(|(index, job)| {
            let id = job_id(index, &job);
            (!completed.contains(id.as_str())).then_some(job)
        })
        .collect()
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

    let run_id = config.effective_run_id();
    let execution = config.execution;
    let model_label = executor.model_label().map(ToOwned::to_owned);
    let mut indexed = futures::stream::iter(jobs.into_iter().enumerate())
        .map(|(index, job)| {
            let run_id = run_id.clone();
            let model_label = model_label.clone();
            async move {
                let id = job_id(index, &job);
                let session_id = match execution {
                    BatchExecutionMode::Local => None,
                    BatchExecutionMode::Daemon => executor
                        .session_id_for(&run_id, &id, &job)
                        .or_else(|| Some(default_daemon_session_id(&run_id, &id))),
                };
                let objective = objective_receipt(&job, None);
                let metadata = normalized_job_metadata(
                    &run_id,
                    execution,
                    session_id.as_deref(),
                    model_label.as_deref(),
                    &job,
                    &objective,
                );
                let result = match executor.execute_batch_job(&run_id, &id, session_id.as_deref(), &job).await {
                    Ok(response) => {
                        let objective = objective_receipt(&job, Some(&response));
                        let metadata = normalized_job_metadata(
                            &run_id,
                            execution,
                            session_id.as_deref(),
                            model_label.as_deref(),
                            &job,
                            &objective,
                        );
                        BatchJobResult {
                            id,
                            status: BatchJobStatus::Succeeded,
                            prompt: Some(job.prompt),
                            response: Some(response),
                            error: None,
                            session_id,
                            objective: Some(objective),
                            metadata: Some(metadata),
                        }
                    }
                    Err(error) => BatchJobResult {
                        id,
                        status: BatchJobStatus::Failed,
                        prompt: Some(job.prompt),
                        response: None,
                        error: Some(sanitize_error(&error)),
                        session_id,
                        objective: Some(objective),
                        metadata: Some(metadata),
                    },
                };
                (index, result)
            }
        })
        .buffer_unordered(config.concurrency)
        .collect::<Vec<_>>()
        .await;
    indexed.sort_by_key(|(index, _)| *index);
    let results: Vec<_> = indexed.into_iter().map(|(_, result)| result).collect();
    let succeeded = results.iter().filter(|result| result.status == BatchJobStatus::Succeeded).count();
    let failed = results.iter().filter(|result| result.status == BatchJobStatus::Failed).count();
    let skipped = results.iter().filter(|result| result.status == BatchJobStatus::Skipped).count();
    let summary = BatchRunSummary {
        source: "batch_trajectory_runner".to_string(),
        status: (if failed == 0 { "ok" } else { "partial" }).to_string(),
        run_id,
        total: results.len(),
        succeeded,
        failed,
        skipped,
        concurrency: config.concurrency,
        format: config.format,
        execution,
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

pub fn render_trajectory_results(
    format: TrajectoryFormat,
    results: &[BatchJobResult],
) -> Result<String, serde_json::Error> {
    match format {
        TrajectoryFormat::Jsonl => results_jsonl(results),
        TrajectoryFormat::Sharegpt => serde_json::to_string_pretty(
            &results
                .iter()
                .map(|result| {
                    json!({
                        "id": result.id,
                        "status": result.status,
                        "conversations": [
                            {"from": "human", "value": result.prompt.clone().unwrap_or_default()},
                            {"from": "assistant", "value": result.response.clone().unwrap_or_default()}
                        ],
                        "metadata": result.metadata,
                        "error": result.error,
                    })
                })
                .collect::<Vec<_>>(),
        )
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        }),
        TrajectoryFormat::EvalJsonl => eval_results_jsonl(results),
    }
}

pub fn eval_results_jsonl(results: &[BatchJobResult]) -> Result<String, serde_json::Error> {
    let mut output = String::new();
    for result in results {
        let metadata = result.metadata.as_ref();
        let record = json!({
            "run_id": metadata.and_then(|value| value.get("run_id")).cloned(),
            "job_id": result.id,
            "status": result.status,
            "prompt": result.prompt,
            "response": result.response,
            "session_id": result.session_id,
            "model": metadata.and_then(|value| value.get("model")).cloned(),
            "execution": metadata.and_then(|value| value.get("execution")).cloned(),
            "redaction": metadata.and_then(|value| value.get("redaction")).cloned(),
            "objective": result.objective,
            "error": result.error,
        });
        output.push_str(&serde_json::to_string(&record)?);
        output.push('\n');
    }
    Ok(output)
}

pub fn batch_run_metadata(summary: &BatchRunSummary, output: &Path) -> Value {
    json!({
        "source": "batch_trajectory_runner",
        "status": summary.status,
        "run_id": summary.run_id,
        "total": summary.total,
        "succeeded": summary.succeeded,
        "failed": summary.failed,
        "skipped": summary.skipped,
        "concurrency": summary.concurrency,
        "format": summary.format,
        "execution": summary.execution,
        "output_file": output.file_name().and_then(|name| name.to_str()),
        "redaction": "safe_metadata_only",
    })
}

pub fn build_run_manifest(summary: &BatchRunSummary, results: &[BatchJobResult]) -> BatchRunManifest {
    BatchRunManifest {
        source: "batch_trajectory_runner".to_string(),
        run_id: summary.run_id.clone(),
        status: summary.status.clone(),
        execution: summary.execution,
        total: summary.total,
        succeeded: summary.succeeded,
        failed: summary.failed,
        skipped: summary.skipped,
        completed_job_ids: results
            .iter()
            .filter(|result| result.status == BatchJobStatus::Succeeded)
            .map(|result| result.id.clone())
            .collect(),
        failed_job_ids: results
            .iter()
            .filter(|result| result.status == BatchJobStatus::Failed)
            .map(|result| result.id.clone())
            .collect(),
        session_ids: results.iter().filter_map(|result| result.session_id.clone()).collect(),
        redaction: "safe_metadata_only".to_string(),
    }
}

fn normalized_job_metadata(
    run_id: &str,
    execution: BatchExecutionMode,
    session_id: Option<&str>,
    model_label: Option<&str>,
    job: &BatchJob,
    objective: &ObjectiveReceipt,
) -> Value {
    json!({
        "source": "batch_trajectory_runner",
        "run_id": run_id,
        "execution": execution,
        "session_id": session_id,
        "model": model_label,
        "has_metadata": job.metadata.is_some(),
        "prompt_chars": job.prompt.chars().count(),
        "objective_status": objective.status,
        "objective_metric": objective.metric,
        "redaction": "safe_metadata_only",
    })
}

fn objective_receipt(job: &BatchJob, response: Option<&str>) -> ObjectiveReceipt {
    let Some(expected) =
        job.metadata.as_ref().and_then(|metadata| metadata.get("expected_contains")).and_then(Value::as_str)
    else {
        return ObjectiveReceipt {
            status: "not_configured".to_string(),
            score: None,
            metric: "expected_contains".to_string(),
            redaction: "objective_label_only".to_string(),
        };
    };
    let score = response.map(|body| if body.contains(expected) { 1.0 } else { 0.0 });
    ObjectiveReceipt {
        status: (if score.is_some() { "scored" } else { "pending" }).to_string(),
        score,
        metric: "expected_contains".to_string(),
        redaction: "objective_label_only".to_string(),
    }
}

fn validate_local_path(path: &Path, err: BatchPolicyError) -> Result<(), BatchPolicyError> {
    let rendered = path.to_string_lossy();
    if rendered.starts_with("http://") || rendered.starts_with("https://") || rendered.starts_with("s3://") {
        return Err(err);
    }
    Ok(())
}

fn job_id(index: usize, job: &BatchJob) -> String {
    job.id.clone().unwrap_or_else(|| format!("line-{}", index + 1))
}

fn default_daemon_session_id(run_id: &str, job_id: &str) -> String {
    format!("batch-{run_id}-{}", sanitize_id_component(job_id))
}

fn stable_run_id(input: &Path, output: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.to_string_lossy().as_bytes());
    hasher.update(b"\0");
    hasher.update(output.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    format!("run-{}", hex::encode(&digest[..6]))
}

fn is_safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn sanitize_id_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .take(80)
        .collect()
}

fn sanitize_error(error: &str) -> String {
    error.lines().next().unwrap_or_default().chars().take(240).collect()
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

        fn model_label(&self) -> Option<&str> {
            Some("fake-model")
        }
    }

    #[tokio::test]
    async fn runtime_facade_batch_executor_matches_existing_batch_semantics() {
        let local_cfg = BatchRunConfig::new("prompts.jsonl", "out", 2, TrajectoryFormat::Jsonl, false);
        let jobs =
            parse_jsonl_jobs(r#"{"id":"a","prompt":"hello runtime","metadata":{"expected_contains":"hello runtime"}}"#)
                .unwrap();

        let (_existing_summary, existing) = run_batch_jobs(&local_cfg, jobs.clone(), &EchoExecutor).await.unwrap();
        let (_runtime_summary, runtime) =
            run_batch_jobs(&local_cfg, jobs.clone(), &RuntimeFacadeBatchExecutor).await.unwrap();

        assert_eq!(existing[0].status, runtime[0].status);
        assert_eq!(existing[0].id, runtime[0].id);
        assert_eq!(existing[0].prompt, runtime[0].prompt);
        assert_eq!(existing[0].response, runtime[0].response);
        assert_eq!(runtime[0].metadata.as_ref().unwrap().get("execution").unwrap(), "local");
        assert!(!runtime[0].metadata.as_ref().unwrap().to_string().contains("hello runtime"));

        let daemon_cfg = BatchRunConfig::new("prompts.jsonl", "out", 2, TrajectoryFormat::EvalJsonl, false)
            .with_execution(BatchExecutionMode::Daemon)
            .with_run_id(Some("runtime-parity".to_string()));
        let (_existing_daemon_summary, existing_daemon) =
            run_batch_jobs(&daemon_cfg, jobs.clone(), &EchoExecutor).await.unwrap();
        let (_runtime_daemon_summary, runtime_daemon) =
            run_batch_jobs(&daemon_cfg, jobs, &RuntimeFacadeBatchExecutor).await.unwrap();

        assert_eq!(existing_daemon[0].response, runtime_daemon[0].response);
        assert_eq!(existing_daemon[0].session_id, runtime_daemon[0].session_id);
        assert_eq!(runtime_daemon[0].session_id.as_deref(), Some("batch-runtime-parity-a"));
        assert_eq!(runtime_daemon[0].objective.as_ref().unwrap().status, "scored");
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

    #[tokio::test]
    async fn daemon_mode_records_session_ids_and_manifest() {
        let cfg = BatchRunConfig::new("prompts.jsonl", "out", 2, TrajectoryFormat::EvalJsonl, false)
            .with_execution(BatchExecutionMode::Daemon)
            .with_run_id(Some("eval-run".to_string()));
        let jobs = parse_jsonl_jobs(r#"{"id":"a","prompt":"hello","metadata":{"expected_contains":"hello"}}"#).unwrap();
        let (summary, results) = run_batch_jobs(&cfg, jobs, &EchoExecutor).await.unwrap();
        let manifest = build_run_manifest(&summary, &results);

        assert_eq!(summary.execution, BatchExecutionMode::Daemon);
        assert_eq!(results[0].session_id.as_deref(), Some("batch-eval-run-a"));
        assert_eq!(results[0].objective.as_ref().unwrap().status, "scored");
        assert_eq!(results[0].objective.as_ref().unwrap().score, Some(1.0));
        assert_eq!(manifest.completed_job_ids, vec!["a"]);
        assert_eq!(manifest.session_ids, vec!["batch-eval-run-a"]);
        assert!(!serde_json::to_string(&manifest).unwrap().contains("hello"));
    }

    #[test]
    fn resume_filter_skips_completed_manifest_jobs() {
        let jobs = parse_jsonl_jobs(
            r#"{"id":"done","prompt":"one"}
{"id":"retry","prompt":"two"}"#,
        )
        .unwrap();
        let manifest = BatchRunManifest {
            source: "batch_trajectory_runner".to_string(),
            run_id: "run".to_string(),
            status: "partial".to_string(),
            execution: BatchExecutionMode::Daemon,
            total: 2,
            succeeded: 1,
            failed: 1,
            skipped: 0,
            completed_job_ids: vec!["done".to_string()],
            failed_job_ids: vec!["retry".to_string()],
            session_ids: vec!["batch-run-done".to_string()],
            redaction: "safe_metadata_only".to_string(),
        };

        let remaining = filter_resume_jobs(jobs, Some(&manifest));
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id.as_deref(), Some("retry"));
    }

    #[test]
    fn renders_result_jsonl() {
        let result = BatchJobResult {
            id: "a".to_string(),
            status: BatchJobStatus::Succeeded,
            prompt: Some("prompt".to_string()),
            response: Some("ok".to_string()),
            error: None,
            session_id: None,
            objective: None,
            metadata: None,
        };
        let rendered = results_jsonl(&[result]).unwrap();
        assert!(rendered.contains(r#""id":"a""#));
        assert!(rendered.ends_with('\n'));
    }

    #[test]
    fn renders_sharegpt_export() {
        let result = BatchJobResult {
            id: "a".to_string(),
            status: BatchJobStatus::Succeeded,
            prompt: Some("ask".to_string()),
            response: Some("ok".to_string()),
            error: None,
            session_id: None,
            objective: None,
            metadata: None,
        };
        let rendered = render_trajectory_results(TrajectoryFormat::Sharegpt, &[result]).unwrap();
        assert!(rendered.contains(r#""conversations""#));
        assert!(rendered.contains(r#""from": "human""#));
        assert!(rendered.contains(r#""value": "ask""#));
        assert!(rendered.contains(r#""from": "assistant""#));
        assert!(rendered.contains(r#""value": "ok""#));
    }

    #[test]
    fn run_metadata_is_safe_and_structured() {
        let summary = BatchRunSummary {
            source: "batch_trajectory_runner".to_string(),
            status: "ok".to_string(),
            run_id: "safe-run".to_string(),
            total: 2,
            succeeded: 2,
            failed: 0,
            skipped: 0,
            concurrency: 2,
            format: TrajectoryFormat::Jsonl,
            execution: BatchExecutionMode::Local,
        };
        let metadata = batch_run_metadata(&summary, Path::new("/tmp/secret/prompts-output.jsonl"));
        assert_eq!(metadata["source"], "batch_trajectory_runner");
        assert_eq!(metadata["total"], 2);
        assert_eq!(metadata["output_file"], "prompts-output.jsonl");
        assert!(!metadata.to_string().contains("/tmp/secret"));
    }

    #[tokio::test]
    async fn batch_eval_runner_kit_fixture_validates_manifest_resume_and_redaction() {
        let cfg = BatchRunConfig::new("fixtures/eval.jsonl", "out/eval.jsonl", 2, TrajectoryFormat::EvalJsonl, true)
            .with_execution(BatchExecutionMode::Daemon)
            .with_run_id(Some("brick-eval".to_string()));
        let jobs = parse_jsonl_jobs(
            r#"{"id":"done","prompt":"already completed","metadata":{"expected_contains":"completed"}}
{"id":"retry","prompt":"secret token should not enter metadata","metadata":{"expected_contains":"echo"}}"#,
        )
        .unwrap();
        let previous_manifest = BatchRunManifest {
            source: "batch_trajectory_runner".to_string(),
            run_id: "brick-eval".to_string(),
            status: "partial".to_string(),
            execution: BatchExecutionMode::Daemon,
            total: 2,
            succeeded: 1,
            failed: 1,
            skipped: 0,
            completed_job_ids: vec!["done".to_string()],
            failed_job_ids: vec!["retry".to_string()],
            session_ids: vec!["batch-brick-eval-done".to_string()],
            redaction: "safe_metadata_only".to_string(),
        };

        let remaining = filter_resume_jobs(jobs, Some(&previous_manifest));
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id.as_deref(), Some("retry"));

        let (summary, results) = run_batch_jobs(&cfg, remaining, &EchoExecutor).await.unwrap();
        let manifest = build_run_manifest(&summary, &results);
        let rendered = render_trajectory_results(TrajectoryFormat::EvalJsonl, &results).unwrap();

        assert_eq!(summary.status, "ok");
        assert_eq!(summary.execution, BatchExecutionMode::Daemon);
        assert_eq!(manifest.completed_job_ids, vec!["retry"]);
        assert_eq!(manifest.session_ids, vec!["batch-brick-eval-retry"]);
        assert_eq!(results[0].objective.as_ref().unwrap().status, "scored");
        assert_eq!(results[0].objective.as_ref().unwrap().score, Some(1.0));
        assert!(rendered.contains(r#""run_id":"brick-eval""#));
        assert!(rendered.contains(r#""redaction":"safe_metadata_only""#));
        assert!(!serde_json::to_string(&manifest).unwrap().contains("secret token"));
        assert!(!results[0].metadata.as_ref().unwrap().to_string().contains("secret token"));

        let remote = BatchRunConfig::new(
            "s3://bucket/eval.jsonl",
            "out/eval.jsonl",
            DEFAULT_BATCH_CONCURRENCY,
            TrajectoryFormat::EvalJsonl,
            true,
        );
        assert_eq!(remote.validate().unwrap_err(), BatchPolicyError::RemoteInputUnsupported);
    }
}
