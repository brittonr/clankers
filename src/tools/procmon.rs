//! Procmon tool — inspect active and historical child processes

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;
use crate::procmon::{ProcessMonitorHandle, ProcessState};

pub struct ProcmonTool {
    definition: ToolDefinition,
    monitor: Option<ProcessMonitorHandle>,
}

impl ProcmonTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "procmon".to_string(),
                description: "Inspect active and historical child processes spawned by tools. Actions: list (active processes), summary (aggregate stats), history (completed processes)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["list", "summary", "history"],
                            "description": "What to show: list (active processes), summary (one-line stats), history (completed)"
                        }
                    },
                    "required": ["action"]
                }),
            },
            monitor: None,
        }
    }

    /// Attach a process monitor.
    pub fn with_monitor(mut self, monitor: ProcessMonitorHandle) -> Self {
        self.monitor = Some(monitor);
        self
    }
}

impl Default for ProcmonTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ProcmonTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> ToolResult {
        let monitor = match &self.monitor {
            Some(m) => m,
            None => return ToolResult::error("Process monitoring not available"),
        };

        let action = match params.get("action").and_then(|v| v.as_str()) {
            Some(a) => a,
            None => return ToolResult::error("Missing required parameter: action"),
        };

        match action {
            "list" => {
                let snapshot = monitor.snapshot();
                if snapshot.is_empty() {
                    return ToolResult::text("No active processes");
                }

                let mut lines = Vec::new();
                lines.push(format!(
                    "{:<8} {:<7} {:<9} {:<9} {}",
                    "PID", "CPU%", "MEM(MB)", "TIME", "COMMAND"
                ));
                lines.push("─".repeat(80));

                for (pid, proc) in &snapshot {
                    let last_sample = proc.snapshots.last();
                    let cpu = last_sample.map(|s| s.cpu_percent).unwrap_or(0.0);
                    let rss_mb = last_sample.map(|s| s.rss_bytes / 1_024 / 1_024).unwrap_or(0);
                    let elapsed = proc.start_time.elapsed();
                    let time_str = format!("{}:{:02}", elapsed.as_secs() / 60, elapsed.as_secs() % 60);

                    lines.push(format!(
                        "{:<8} {:<7.1} {:<9} {:<9} {}",
                        pid, cpu, rss_mb, time_str, proc.meta.command
                    ));

                    // Show children indented
                    for child_pid in &proc.children {
                        if let Some((_, child_proc)) = snapshot.iter().find(|(p, _)| p == child_pid) {
                            let child_sample = child_proc.snapshots.last();
                            let child_cpu = child_sample.map(|s| s.cpu_percent).unwrap_or(0.0);
                            let child_rss_mb = child_sample.map(|s| s.rss_bytes / 1_024 / 1_024).unwrap_or(0);
                            let child_elapsed = child_proc.start_time.elapsed();
                            let child_time_str =
                                format!("{}:{:02}", child_elapsed.as_secs() / 60, child_elapsed.as_secs() % 60);

                            lines.push(format!(
                                " └─ {:<5} {:<7.1} {:<9} {:<9} {}",
                                child_pid, child_cpu, child_rss_mb, child_time_str, child_proc.meta.command
                            ));
                        } else {
                            lines.push(format!(" └─ {:<5} (not tracked)", child_pid));
                        }
                    }
                }

                ToolResult::text(lines.join("\n"))
            }
            "summary" => {
                let stats = monitor.aggregate();
                let msg = format!(
                    "{} active processes | {} finished | Total: {:.1} MB RSS, {:.1}% CPU",
                    stats.active_count,
                    stats.finished_count,
                    stats.total_rss as f64 / 1_024.0 / 1_024.0,
                    stats.total_cpu_percent
                );
                ToolResult::text(msg)
            }
            "history" => {
                let history = monitor.history();
                if history.is_empty() {
                    return ToolResult::text("No finished processes");
                }

                let mut lines = Vec::new();
                lines.push(format!(
                    "{:<8} {:<6} {:<10} {:<9} {}",
                    "PID", "EXIT", "PEAK(MB)", "WALL", "COMMAND"
                ));
                lines.push("─".repeat(80));

                for (pid, proc) in history {
                    let exit_code = match proc.state {
                        ProcessState::Exited { code, .. } => code.map(|c| c.to_string()).unwrap_or("?".to_string()),
                        ProcessState::Running => "RUN".to_string(),
                    };
                    let wall_time = match proc.state {
                        ProcessState::Exited { wall_time, .. } => wall_time,
                        ProcessState::Running => proc.start_time.elapsed(),
                    };
                    let wall_str = format!("{}:{:02}", wall_time.as_secs() / 60, wall_time.as_secs() % 60);
                    let peak_mb = proc.peak_rss / 1_024 / 1_024;

                    lines.push(format!(
                        "{:<8} {:<6} {:<10} {:<9} {}",
                        pid, exit_code, peak_mb, wall_str, proc.meta.command
                    ));
                }

                ToolResult::text(lines.join("\n"))
            }
            _ => ToolResult::error(format!("Unknown action: {}", action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_tool_has_correct_name() {
        let tool = ProcmonTool::new();
        assert_eq!(tool.definition().name, "procmon");
    }

    #[test]
    fn test_definition_has_required_fields() {
        let tool = ProcmonTool::new();
        let def = tool.definition();
        assert!(!def.description.is_empty());
        assert!(def.input_schema.get("properties").is_some());
    }
}
