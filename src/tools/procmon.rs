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
                            "enum": ["list", "summary", "inspect", "history"],
                            "description": "What to show: list (active processes), summary (one-line stats), inspect (deep-dive on one PID), history (completed)"
                        },
                        "pid": {
                            "type": "number",
                            "description": "Process ID to inspect (required for inspect action)"
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

        let pid_param = params.get("pid").and_then(|v| v.as_u64()).map(|v| v as u32);

        match action {
            "list" => Self::format_process_list(monitor),
            "summary" => Self::format_summary(monitor),
            "history" => Self::format_history(monitor),
            "inspect" => Self::format_inspect(monitor, pid_param),
            _ => ToolResult::error(format!("Unknown action: {}", action)),
        }
    }
}

impl ProcmonTool {
    /// Format the active process list with CPU, memory, and child processes.
    fn format_process_list(monitor: &ProcessMonitorHandle) -> ToolResult {
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

    /// Format aggregate process statistics (active, finished, CPU, memory).
    fn format_summary(monitor: &ProcessMonitorHandle) -> ToolResult {
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

    /// Format the history of completed processes.
    fn format_history(monitor: &ProcessMonitorHandle) -> ToolResult {
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

    /// Format detailed inspection of a specific process by PID.
    fn format_inspect(monitor: &ProcessMonitorHandle, pid_param: Option<u32>) -> ToolResult {
        let pid = match pid_param {
            Some(p) => p,
            None => return ToolResult::error("inspect requires a 'pid' parameter"),
        };

        // Search active first, then history
        let snapshot = monitor.snapshot();
        let history = monitor.history();

        let (found_pid, proc, source) = if let Some((_, p)) = snapshot.iter().find(|(p, _)| *p == pid) {
            (pid, p.clone(), "active")
        } else if let Some((_, p)) = history.iter().find(|(p, _)| *p == pid) {
            (pid, p.clone(), "history")
        } else {
            return ToolResult::error(format!("No process found with PID {}", pid));
        };

        let last_sample = proc.snapshots.last();
        let cpu = last_sample.map(|s| s.cpu_percent).unwrap_or(0.0);
        let rss_mb = last_sample.map(|s| s.rss_bytes as f64 / 1_048_576.0).unwrap_or(0.0);
        let peak_mb = proc.peak_rss as f64 / 1_048_576.0;
        let elapsed = match &proc.state {
            ProcessState::Running => proc.start_time.elapsed(),
            ProcessState::Exited { wall_time, .. } => *wall_time,
        };
        let time_str = format!("{}:{:02}", elapsed.as_secs() / 60, elapsed.as_secs() % 60);

        let state_str = match &proc.state {
            ProcessState::Running => "Running".to_string(),
            ProcessState::Exited { code, wall_time } => {
                let code_str = code.map(|c| c.to_string()).unwrap_or_else(|| "?".to_string());
                format!("Exited (code {}, wall {}:{:02})", code_str, wall_time.as_secs() / 60, wall_time.as_secs() % 60)
            }
        };

        let cpu_values: Vec<f32> = proc.snapshots.iter().map(|s| s.cpu_percent).collect();
        let mem_values: Vec<f32> = proc.snapshots.iter().map(|s| s.rss_bytes as f32).collect();
        let cpu_spark = sparkline(&cpu_values, 100.0, 40);
        let mem_spark = sparkline(&mem_values, proc.peak_rss as f32, 40);

        let children_str = if proc.children.is_empty() {
            "none".to_string()
        } else {
            proc.children.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ")
        };

        let lines = vec![
            format!("PID:       {}", found_pid),
            format!("Command:   {}", proc.meta.command),
            format!("Tool:      {}", proc.meta.tool_name),
            format!("Call ID:   {}", proc.meta.call_id),
            format!("State:     {}", state_str),
            format!("Source:    {}", source),
            format!("Wall time: {}", time_str),
            format!("CPU:       {:.1}%", cpu),
            format!("RSS:       {:.1} MB", rss_mb),
            format!("Peak RSS:  {:.1} MB", peak_mb),
            format!("Samples:   {}", proc.snapshots.len()),
            format!("Children:  {}", children_str),
            String::new(),
            format!("CPU history:  {}", cpu_spark),
            format!("Mem history:  {}", mem_spark),
        ];

        ToolResult::text(lines.join("\n"))
    }
}

/// Render values as a Unicode sparkline using block characters.
///
/// Maps each value to one of 8 block levels (▁▂▃▄▅▆▇█) relative to `max_val`.
/// Takes the last `width` values if there are more than `width`.
fn sparkline(values: &[f32], max_val: f32, width: usize) -> String {
    const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

    if values.is_empty() {
        return "(no data)".to_string();
    }

    let start = values.len().saturating_sub(width);
    let slice = &values[start..];

    slice
        .iter()
        .map(|&v| {
            if max_val <= 0.0 {
                BLOCKS[0]
            } else {
                let ratio = (v / max_val).clamp(0.0, 1.0);
                let idx = (ratio * 7.0).round() as usize;
                BLOCKS[idx.min(7)]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;
    use std::time::Instant;

    use tokio_util::sync::CancellationToken;

    use crate::procmon::{ProcessMonitor, ProcessMonitorConfig, ProcessMeta, ResourceSnapshot};
    use crate::tools::ToolResultContent;

    fn test_ctx() -> ToolContext {
        ToolContext::new("test-call".to_string(), CancellationToken::new(), None)
    }

    fn test_meta(cmd: &str) -> ProcessMeta {
        ProcessMeta {
            tool_name: "bash".to_string(),
            command: cmd.to_string(),
            call_id: format!("call-{}", cmd),
        }
    }

    fn make_tool_with_monitor() -> (ProcmonTool, Arc<ProcessMonitor>) {
        let monitor = Arc::new(ProcessMonitor::new(ProcessMonitorConfig::default(), None));
        let tool = ProcmonTool::new().with_monitor(Arc::clone(&monitor));
        (tool, monitor)
    }

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
        // inspect should be in the enum
        let actions = def.input_schema["properties"]["action"]["enum"].as_array().expect("action enum should exist");
        let names: Vec<&str> = actions.iter().map(|v| v.as_str().expect("action should be string")).collect();
        assert!(names.contains(&"inspect"));
        // pid property should exist
        assert!(def.input_schema["properties"]["pid"].is_object());
    }

    // ── sparkline tests ─────────────────────────────────────────────────

    #[test]
    fn test_sparkline_empty() {
        assert_eq!(sparkline(&[], 100.0, 10), "(no data)");
    }

    #[test]
    fn test_sparkline_all_zeros() {
        let vals = vec![0.0; 5];
        let s = sparkline(&vals, 100.0, 10);
        assert_eq!(s, "▁▁▁▁▁");
    }

    #[test]
    fn test_sparkline_all_max() {
        let vals = vec![100.0; 4];
        let s = sparkline(&vals, 100.0, 10);
        assert_eq!(s, "████");
    }

    #[test]
    fn test_sparkline_zero_max() {
        let vals = vec![50.0, 100.0];
        let s = sparkline(&vals, 0.0, 10);
        // All should map to lowest block when max is 0
        assert_eq!(s, "▁▁");
    }

    #[test]
    fn test_sparkline_truncates_to_width() {
        let vals: Vec<f32> = (0..20).map(|i| i as f32).collect();
        let s = sparkline(&vals, 19.0, 5);
        assert_eq!(s.chars().count(), 5);
    }

    #[test]
    fn test_sparkline_ascending() {
        let vals = vec![0.0, 50.0, 100.0];
        let s = sparkline(&vals, 100.0, 10);
        let chars: Vec<char> = s.chars().collect();
        assert_eq!(chars.len(), 3);
        assert_eq!(chars[0], '▁');
        assert_eq!(chars[2], '█');
    }

    // ── tool action tests ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_no_monitor_returns_error() {
        let tool = ProcmonTool::new();
        let result = tool.execute(&test_ctx(), json!({"action": "list"})).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_missing_action_returns_error() {
        let (tool, _monitor) = make_tool_with_monitor();
        let result = tool.execute(&test_ctx(), json!({})).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_unknown_action_returns_error() {
        let (tool, _monitor) = make_tool_with_monitor();
        let result = tool.execute(&test_ctx(), json!({"action": "nope"})).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_list_empty() {
        let (tool, _monitor) = make_tool_with_monitor();
        let result = tool.execute(&test_ctx(), json!({"action": "list"})).await;
        let text = match &result.content[0] {
            ToolResultContent::Text { text } => text.as_str(),
            _ => panic!("expected text"),
        };
        assert!(text.contains("No active processes"));
    }

    #[tokio::test]
    async fn test_list_with_processes() {
        let (tool, monitor) = make_tool_with_monitor();
        monitor.register(111, test_meta("cargo build"));
        monitor.register(222, test_meta("npm test"));
        monitor.inject_snapshot(111, ResourceSnapshot {
            cpu_percent: 42.5,
            rss_bytes: 100 * 1_048_576, // 100 MB
            timestamp: Instant::now(),
        });

        let result = tool.execute(&test_ctx(), json!({"action": "list"})).await;
        let text = match &result.content[0] {
            ToolResultContent::Text { text } => text.as_str(),
            _ => panic!("expected text"),
        };
        assert!(text.contains("111"));
        assert!(text.contains("cargo build"));
        assert!(text.contains("222"));
        assert!(text.contains("npm test"));
    }

    #[tokio::test]
    async fn test_summary_with_processes() {
        let (tool, monitor) = make_tool_with_monitor();
        monitor.register(111, test_meta("cargo build"));
        monitor.inject_snapshot(111, ResourceSnapshot {
            cpu_percent: 50.0,
            rss_bytes: 200 * 1_048_576,
            timestamp: Instant::now(),
        });

        let result = tool.execute(&test_ctx(), json!({"action": "summary"})).await;
        let text = match &result.content[0] {
            ToolResultContent::Text { text } => text.as_str(),
            _ => panic!("expected text"),
        };
        assert!(text.contains("1 active"));
        assert!(text.contains("0 finished"));
    }

    #[tokio::test]
    async fn test_history_empty() {
        let (tool, _monitor) = make_tool_with_monitor();
        let result = tool.execute(&test_ctx(), json!({"action": "history"})).await;
        let text = match &result.content[0] {
            ToolResultContent::Text { text } => text.as_str(),
            _ => panic!("expected text"),
        };
        assert!(text.contains("No finished processes"));
    }

    #[tokio::test]
    async fn test_history_with_finished() {
        let (tool, monitor) = make_tool_with_monitor();
        monitor.register(333, test_meta("make test"));
        monitor.inject_snapshot(333, ResourceSnapshot {
            cpu_percent: 80.0,
            rss_bytes: 50 * 1_048_576,
            timestamp: Instant::now(),
        });
        monitor.mark_exited(333, Some(0));

        let result = tool.execute(&test_ctx(), json!({"action": "history"})).await;
        let text = match &result.content[0] {
            ToolResultContent::Text { text } => text.as_str(),
            _ => panic!("expected text"),
        };
        assert!(text.contains("333"));
        assert!(text.contains("make test"));
        assert!(text.contains("0")); // exit code
    }

    #[tokio::test]
    async fn test_inspect_missing_pid() {
        let (tool, _monitor) = make_tool_with_monitor();
        let result = tool.execute(&test_ctx(), json!({"action": "inspect"})).await;
        assert!(result.is_error);
        let text = match &result.content[0] {
            ToolResultContent::Text { text } => text.as_str(),
            _ => panic!("expected text"),
        };
        assert!(text.contains("pid"));
    }

    #[tokio::test]
    async fn test_inspect_not_found() {
        let (tool, _monitor) = make_tool_with_monitor();
        let result = tool.execute(&test_ctx(), json!({"action": "inspect", "pid": 99999})).await;
        assert!(result.is_error);
        let text = match &result.content[0] {
            ToolResultContent::Text { text } => text.as_str(),
            _ => panic!("expected text"),
        };
        assert!(text.contains("99999"));
    }

    #[tokio::test]
    async fn test_inspect_active_process() {
        let (tool, monitor) = make_tool_with_monitor();
        monitor.register(555, ProcessMeta {
            tool_name: "bash".to_string(),
            command: "cargo build --release".to_string(),
            call_id: "call-555".to_string(),
        });
        for i in 0..5 {
            monitor.inject_snapshot(555, ResourceSnapshot {
                cpu_percent: (i as f32) * 20.0,
                rss_bytes: (i + 1) as u64 * 50 * 1_048_576,
                timestamp: Instant::now(),
            });
        }
        monitor.add_child(555, 600);
        monitor.add_child(555, 601);

        let result = tool.execute(&test_ctx(), json!({"action": "inspect", "pid": 555})).await;
        assert!(!result.is_error);
        let text = match &result.content[0] {
            ToolResultContent::Text { text } => text.as_str(),
            _ => panic!("expected text"),
        };
        assert!(text.contains("PID:       555"));
        assert!(text.contains("cargo build --release"));
        assert!(text.contains("call-555"));
        assert!(text.contains("Running"));
        assert!(text.contains("active"));
        assert!(text.contains("Samples:   5"));
        assert!(text.contains("600"));
        assert!(text.contains("601"));
        assert!(text.contains("CPU history:"));
        assert!(text.contains("Mem history:"));
    }

    #[tokio::test]
    async fn test_inspect_finished_process() {
        let (tool, monitor) = make_tool_with_monitor();
        monitor.register(777, test_meta("pytest"));
        monitor.inject_snapshot(777, ResourceSnapshot {
            cpu_percent: 90.0,
            rss_bytes: 300 * 1_048_576,
            timestamp: Instant::now(),
        });
        monitor.mark_exited(777, Some(1));

        let result = tool.execute(&test_ctx(), json!({"action": "inspect", "pid": 777})).await;
        assert!(!result.is_error);
        let text = match &result.content[0] {
            ToolResultContent::Text { text } => text.as_str(),
            _ => panic!("expected text"),
        };
        assert!(text.contains("777"));
        assert!(text.contains("Exited"));
        assert!(text.contains("code 1"));
        assert!(text.contains("history")); // source
    }
}
