//! Autoresearch experiment dashboard widget.

use std::path::Path;

use ratatui::prelude::*;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;

#[derive(Debug, Clone, Default)]
pub struct ExperimentDashboardState {
    pub visible: bool,
    pub title: String,
    pub metric_name: String,
    pub best_metric: Option<f64>,
    pub total_runs: usize,
    pub kept: usize,
    pub discarded: usize,
    pub crashed: usize,
    pub rows: Vec<ExperimentDashboardRow>,
}

#[derive(Debug, Clone)]
pub struct ExperimentDashboardRow {
    pub run: u32,
    pub status: String,
    pub metric: f64,
    pub description: String,
}

impl ExperimentDashboardState {
    pub fn from_jsonl(path: &Path) -> std::io::Result<Self> {
        let log = clankers_autoresearch::jsonl::read_log(path)?;
        let config = log.config;
        let metric_name = config.as_ref().map(|c| c.metric_name.clone()).unwrap_or_else(|| "metric".to_string());
        let minimize = config.as_ref().is_some_and(|c| c.is_minimize());
        let title = config.as_ref().map(|c| c.name.clone()).unwrap_or_else(|| "Autoresearch".to_string());
        let mut kept = 0usize;
        let mut discarded = 0usize;
        let mut crashed = 0usize;
        let mut best_metric: Option<f64> = None;
        let mut rows = Vec::new();

        for result in &log.results {
            match result.status {
                clankers_autoresearch::ResultStatus::Keep => {
                    kept += 1;
                    best_metric = Some(match best_metric {
                        None => result.metric,
                        Some(best) if minimize => best.min(result.metric),
                        Some(best) => best.max(result.metric),
                    });
                }
                clankers_autoresearch::ResultStatus::Discard => discarded += 1,
                clankers_autoresearch::ResultStatus::Crash | clankers_autoresearch::ResultStatus::ChecksFailed => {
                    crashed += 1
                }
            }
            rows.push(ExperimentDashboardRow {
                run: result.run,
                status: format!("{:?}", result.status).to_lowercase(),
                metric: result.metric,
                description: result.description.clone(),
            });
        }

        Ok(Self {
            visible: false,
            title,
            metric_name,
            best_metric,
            total_runs: rows.len(),
            kept,
            discarded,
            crashed,
            rows,
        })
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }
}

pub fn render_experiment_dashboard(frame: &mut Frame<'_>, area: Rect, state: &ExperimentDashboardState) {
    let mut lines = Vec::new();
    lines.push(Line::from(state.title.clone()));
    lines.push(Line::from(format!(
        "runs={} kept={} discarded={} crashed={}",
        state.total_runs, state.kept, state.discarded, state.crashed
    )));
    let best = state.best_metric.map(|v| format!("{v:.4}")).unwrap_or_else(|| "n/a".to_string());
    lines.push(Line::from(format!("best {}: {}", state.metric_name, best)));
    lines.push(Line::from(""));
    lines.push(Line::from("Run  Status        Metric      Description"));
    for row in state.rows.iter().rev().take(12) {
        lines.push(Line::from(format!("{:>3}  {:<12} {:>10.4}  {}", row.run, row.status, row.metric, row.description)));
    }
    let paragraph = Paragraph::new(lines).block(Block::default().title("Experiments").borders(Borders::ALL));
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_state_parses_jsonl() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("autoresearch.jsonl");
        let config = clankers_autoresearch::ExperimentConfig::new("test", "latency_ms");
        clankers_autoresearch::jsonl::append_config(&path, &config).unwrap();
        let result = clankers_autoresearch::ExperimentResult {
            record_type: "result".to_string(),
            run: 1,
            commit: "abc1234".to_string(),
            metric: 10.0,
            metrics: None,
            status: clankers_autoresearch::ResultStatus::Keep,
            description: "baseline".to_string(),
            asi: None,
            timestamp: chrono::Utc::now(),
        };
        clankers_autoresearch::jsonl::append_result(&path, &result).unwrap();
        let state = ExperimentDashboardState::from_jsonl(&path).unwrap();
        assert_eq!(state.total_runs, 1);
        assert_eq!(state.kept, 1);
        assert_eq!(state.best_metric, Some(10.0));
    }
}
