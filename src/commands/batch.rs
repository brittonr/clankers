use std::path::PathBuf;
use std::process::Stdio;

use crate::cli::BatchAction;
use crate::cli::BatchExecutionArg;
use crate::cli::TrajectoryFormatArg;
use crate::commands::CommandContext;
use crate::error::Error;
use crate::error::Result;
use crate::modes::batch::BatchExecutionMode;
use crate::modes::batch::BatchJob;
use crate::modes::batch::BatchJobExecutor;
use crate::modes::batch::BatchRunConfig;
use crate::modes::batch::BatchRunManifest;
use crate::modes::batch::TrajectoryFormat;
use crate::modes::batch::batch_run_metadata;
use crate::modes::batch::build_run_manifest;
use crate::modes::batch::filter_resume_jobs;
use crate::modes::batch::parse_jsonl_jobs;
use crate::modes::batch::render_trajectory_results;

pub async fn run(ctx: &CommandContext, action: BatchAction) -> Result<()> {
    match action {
        BatchAction::Run {
            input,
            output,
            concurrency,
            format,
            execution,
            run_id,
            resume,
        } => {
            let format = match format {
                TrajectoryFormatArg::Jsonl => TrajectoryFormat::Jsonl,
                TrajectoryFormatArg::Sharegpt => TrajectoryFormat::Sharegpt,
                TrajectoryFormatArg::EvalJsonl => TrajectoryFormat::EvalJsonl,
            };
            let execution = match execution {
                BatchExecutionArg::Local => BatchExecutionMode::Local,
                BatchExecutionArg::Daemon => BatchExecutionMode::Daemon,
            };
            let config = BatchRunConfig::new(&input, &output, concurrency, format, resume)
                .with_execution(execution)
                .with_run_id(run_id);
            config.validate().map_err(|err| Error::Config {
                message: err.to_string(),
            })?;
            let body = tokio::fs::read_to_string(&input).await.map_err(|source| Error::Io { source })?;
            let mut jobs = parse_jsonl_jobs(&body).map_err(|err| Error::Config {
                message: err.to_string(),
            })?;
            let prior_manifest = if resume {
                read_manifest(&manifest_path(&output)).await?
            } else {
                None
            };
            jobs = filter_resume_jobs(jobs, prior_manifest.as_ref());
            let executor = CliPromptExecutor::new(ctx);
            let (summary, results) =
                crate::modes::batch::run_batch_jobs(&config, jobs, &executor).await.map_err(|err| Error::Config {
                    message: err.to_string(),
                })?;
            let rendered = render_trajectory_results(format, &results).map_err(|source| Error::Json { source })?;
            write_output(&output, &rendered, resume).await?;
            write_manifest(&manifest_path(&output), &build_run_manifest(&summary, &results)).await?;
            let metadata = batch_run_metadata(&summary, &output);
            tracing::info!(target: "clankers::batch", %metadata, "batch trajectory run complete");
            println!(
                "batch complete: total={} succeeded={} failed={} output={}",
                summary.total,
                summary.succeeded,
                summary.failed,
                output.display()
            );
            Ok(())
        }
    }
}

struct CliPromptExecutor {
    model: String,
    api_key: Option<String>,
    api_base: Option<String>,
    account: Option<String>,
}

impl CliPromptExecutor {
    fn new(ctx: &CommandContext) -> Self {
        Self {
            model: ctx.model.clone(),
            api_key: ctx.api_key.clone(),
            api_base: ctx.api_base.clone(),
            account: ctx.account.clone(),
        }
    }
}

#[async_trait::async_trait]
impl BatchJobExecutor for CliPromptExecutor {
    async fn execute(&self, job: &BatchJob) -> std::result::Result<String, String> {
        self.execute_batch_job("", "", None, job).await
    }

    async fn execute_batch_job(
        &self,
        _run_id: &str,
        _job_id: &str,
        session_id: Option<&str>,
        job: &BatchJob,
    ) -> std::result::Result<String, String> {
        let current_exe = std::env::current_exe().map_err(|err| err.to_string())?;
        let mut cmd = tokio::process::Command::new(current_exe);
        cmd.arg("--print")
            .arg(&job.prompt)
            .arg("--mode")
            .arg("plain")
            .arg("--tools")
            .arg("none")
            .arg("--model")
            .arg(&self.model)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(api_base) = &self.api_base {
            cmd.arg("--api-base").arg(api_base);
        }
        if let Some(account) = &self.account {
            cmd.arg("--account").arg(account);
        }
        if let Some(session_id) = session_id {
            cmd.arg("--resume").arg(session_id);
        }
        if let Some(api_key) = &self.api_key {
            cmd.env("CLANKERS_API_KEY", api_key);
        }
        let output = cmd.output().await.map_err(|err| err.to_string())?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(stderr.trim().to_string())
        }
    }

    fn model_label(&self) -> Option<&str> {
        Some(&self.model)
    }
}

async fn write_output(path: &PathBuf, rendered: &str, resume: bool) -> Result<()> {
    if resume && tokio::fs::try_exists(path).await.map_err(|source| Error::Io { source })? {
        let mut existing = tokio::fs::read_to_string(path).await.map_err(|source| Error::Io { source })?;
        existing.push_str(rendered);
        tokio::fs::write(path, existing).await.map_err(|source| Error::Io { source })
    } else {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await.map_err(|source| Error::Io { source })?;
            }
        }
        tokio::fs::write(path, rendered).await.map_err(|source| Error::Io { source })
    }
}

fn manifest_path(output: &PathBuf) -> PathBuf {
    let mut path = output.clone();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!("{value}.manifest.json"))
        .unwrap_or_else(|| "manifest.json".to_string());
    path.set_extension(extension);
    path
}

async fn read_manifest(path: &PathBuf) -> Result<Option<BatchRunManifest>> {
    if !tokio::fs::try_exists(path).await.map_err(|source| Error::Io { source })? {
        return Ok(None);
    }
    let body = tokio::fs::read_to_string(path).await.map_err(|source| Error::Io { source })?;
    serde_json::from_str(&body).map(Some).map_err(|source| Error::Json { source })
}

async fn write_manifest(path: &PathBuf, manifest: &BatchRunManifest) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await.map_err(|source| Error::Io { source })?;
        }
    }
    let rendered = serde_json::to_string_pretty(manifest).map_err(|source| Error::Json { source })?;
    tokio::fs::write(path, format!("{rendered}\n")).await.map_err(|source| Error::Io { source })
}
