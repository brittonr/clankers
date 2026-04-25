//! Autoresearch tools: init_experiment, run_experiment, log_experiment.

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;
use tokio::process::Command;
use tokio::time::Duration;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

const DEFAULT_TIMEOUT_SECS: u64 = 600;

pub struct InitExperimentTool {
    definition: ToolDefinition,
}

impl InitExperimentTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "init_experiment".to_string(),
                description:
                    "Initialize an autoresearch experiment session and append a config record to autoresearch.jsonl."
                        .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "metric_name": {"type": "string"},
                        "metric_unit": {"type": "string"},
                        "direction": {"type": "string", "enum": ["minimize", "maximize"]}
                    },
                    "required": ["name", "metric_name"]
                }),
            },
        }
    }
}

#[async_trait]
impl Tool for InitExperimentTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> ToolResult {
        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(v) if !v.is_empty() => v,
            _ => return ToolResult::error("Missing required name"),
        };
        let metric_name = match params.get("metric_name").and_then(|v| v.as_str()) {
            Some(v) if !v.is_empty() => v,
            _ => return ToolResult::error("Missing required metric_name"),
        };
        let metric_unit = params.get("metric_unit").and_then(|v| v.as_str());
        let direction = params.get("direction").and_then(|v| v.as_str()).or(Some("minimize"));
        let cwd = match std::env::current_dir() {
            Ok(cwd) => cwd,
            Err(e) => return ToolResult::error(format!("failed to get cwd: {e}")),
        };
        match clankers_autoresearch::ExperimentSession::init(&cwd, name, metric_name, metric_unit, direction) {
            Ok(session) => ToolResult::text(format!(
                "Initialized autoresearch session '{}' tracking '{}' at {}",
                session.config.name,
                session.config.metric_name,
                session.log_path.display()
            )),
            Err(e) => ToolResult::error(format!("failed to initialize experiment: {e}")),
        }
    }
}

pub struct RunExperimentTool {
    definition: ToolDefinition,
}

impl RunExperimentTool {
    pub fn new() -> Self {
        Self { definition: ToolDefinition {
            name: "run_experiment".to_string(),
            description: "Run an experiment command with timeout, capture output, parse METRIC name=value lines, and optionally run autoresearch.checks.sh.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "timeout_seconds": {"type": "integer"}
                },
                "required": ["command"]
            }),
        }}
    }
}

#[async_trait]
impl Tool for RunExperimentTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> ToolResult {
        let command = match params.get("command").and_then(|v| v.as_str()) {
            Some(v) if !v.is_empty() => v,
            _ => return ToolResult::error("Missing required command"),
        };
        let timeout_secs = params.get("timeout_seconds").and_then(|v| v.as_u64()).unwrap_or(DEFAULT_TIMEOUT_SECS);
        let cwd = match std::env::current_dir() {
            Ok(cwd) => cwd,
            Err(e) => return ToolResult::error(format!("failed to get cwd: {e}")),
        };

        let child = Command::new("sh").arg("-c").arg(command).current_dir(&cwd).output();
        let output = match tokio::time::timeout(Duration::from_secs(timeout_secs), child).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => return ToolResult::error(format!("failed to run command: {e}")),
            Err(_) => return ToolResult::error(format!("command timed out after {timeout_secs}s")),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");
        let metrics = clankers_autoresearch::metrics::extract_metrics(&combined);

        let checks_path = cwd.join("autoresearch.checks.sh");
        let mut checks_status = "not_run".to_string();
        if output.status.success() && checks_path.exists() {
            let checks = Command::new("sh").arg(&checks_path).current_dir(&cwd).output().await;
            checks_status = match checks {
                Ok(out) if out.status.success() => "passed".to_string(),
                Ok(out) => format!("failed({})", out.status.code().unwrap_or(-1)),
                Err(e) => format!("error({e})"),
            };
        }

        let status = if output.status.success() { "success" } else { "failed" };
        ToolResult::text(format!(
            "status: {status}\nexit_code: {}\nchecks: {checks_status}\nmetrics: {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            output.status.code().unwrap_or(-1),
            serde_json::to_string_pretty(&metrics).unwrap_or_default(),
            stdout,
            stderr,
        ))
    }
}

pub struct LogExperimentTool {
    definition: ToolDefinition,
}

impl LogExperimentTool {
    pub fn new() -> Self {
        Self { definition: ToolDefinition {
            name: "log_experiment".to_string(),
            description: "Append an experiment result to autoresearch.jsonl, update best/confidence, commit on keep or revert on discard/crash/checks_failed.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "commit": {"type": "string"},
                    "metric": {"type": "number"},
                    "status": {"type": "string", "enum": ["keep", "discard", "crash", "checks_failed"]},
                    "description": {"type": "string"}
                },
                "required": ["commit", "metric", "status", "description"]
            }),
        }}
    }
}

#[async_trait]
impl Tool for LogExperimentTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> ToolResult {
        let commit = params.get("commit").and_then(|v| v.as_str()).unwrap_or("unknown");
        let metric = match params.get("metric").and_then(|v| v.as_f64()) {
            Some(v) => v,
            None => return ToolResult::error("Missing required numeric metric"),
        };
        let status = match params.get("status").and_then(|v| v.as_str()) {
            Some("keep") => clankers_autoresearch::ResultStatus::Keep,
            Some("discard") => clankers_autoresearch::ResultStatus::Discard,
            Some("crash") => clankers_autoresearch::ResultStatus::Crash,
            Some("checks_failed") => clankers_autoresearch::ResultStatus::ChecksFailed,
            _ => return ToolResult::error("status must be keep|discard|crash|checks_failed"),
        };
        let description = params.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let cwd = match std::env::current_dir() {
            Ok(cwd) => cwd,
            Err(e) => return ToolResult::error(format!("failed to get cwd: {e}")),
        };
        let mut session = match clankers_autoresearch::ExperimentSession::load(&cwd) {
            Ok(session) => session,
            Err(e) => return ToolResult::error(format!("failed to load autoresearch session: {e}")),
        };
        match session.record_result(commit, metric, status, description) {
            Ok(outcome) => {
                let confidence = outcome
                    .confidence
                    .as_ref()
                    .map(|c| format!("{:.2}x", c.score))
                    .unwrap_or_else(|| "n/a".to_string());
                ToolResult::text(format!(
                    "logged run {}\nnew_best: {}\nbest_metric: {:?}\nconfidence: {}",
                    outcome.run, outcome.is_new_best, outcome.best_metric, confidence
                ))
            }
            Err(e) => ToolResult::error(format!("failed to log experiment: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definitions() {
        assert_eq!(InitExperimentTool::new().definition().name, "init_experiment");
        assert_eq!(RunExperimentTool::new().definition().name, "run_experiment");
        assert_eq!(LogExperimentTool::new().definition().name, "log_experiment");
        assert!(InitExperimentTool::new().definition().input_schema.get("properties").is_some());
    }
}
