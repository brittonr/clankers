use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

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
}
