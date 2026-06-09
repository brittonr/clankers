//! Experiment session manager — coordinates JSONL state, metrics, and git.

use std::path::Path;
use std::path::PathBuf;

use chrono::DateTime;
use chrono::Utc;

use crate::confidence;
use crate::jsonl::ExperimentConfig;
use crate::jsonl::ExperimentResult;
use crate::jsonl::ResultStatus;
use crate::jsonl::{self};

#[derive(Debug)]
pub struct ExperimentSession {
    pub config: ExperimentConfig,
    pub log_path: PathBuf,
    pub cwd: PathBuf,
    pub run_counter: u32,
    pub results: Vec<ExperimentResult>,
    pub best_metric: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub struct ExperimentInitOptions<'a> {
    pub cwd: &'a Path,
    pub name: &'a str,
    pub metric_name: &'a str,
    pub metric_unit: Option<&'a str>,
    pub direction: Option<&'a str>,
    pub timestamp: DateTime<Utc>,
}

impl ExperimentSession {
    pub fn init(options: ExperimentInitOptions<'_>) -> std::io::Result<Self> {
        let log_path = options.cwd.join("autoresearch.jsonl");

        let (run_counter, results, best_metric) = if log_path.exists() {
            let log = jsonl::read_log(&log_path)?;
            let run_counter = log.results.last().map(|r| r.run).unwrap_or(0);
            let best = compute_best(&log.results, options.direction == Some("minimize"));
            (run_counter, log.results, best)
        } else {
            (0, Vec::new(), None)
        };

        let mut config = ExperimentConfig::new(options.name, options.metric_name, options.timestamp);
        config.metric_unit = options.metric_unit.map(String::from);
        config.direction = options.direction.map(String::from);

        jsonl::append_config(&log_path, &config)?;

        Ok(Self {
            config,
            log_path,
            cwd: options.cwd.to_path_buf(),
            run_counter,
            results,
            best_metric,
        })
    }

    pub fn load(cwd: &Path) -> std::io::Result<Self> {
        let log_path = cwd.join("autoresearch.jsonl");
        let log = jsonl::read_log(&log_path)?;
        let config = log
            .config
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "autoresearch config not found"))?;
        let run_counter = log.results.last().map(|r| r.run).unwrap_or(0);
        let best_metric = compute_best(&log.results, config.is_minimize());
        Ok(Self {
            config,
            log_path,
            cwd: cwd.to_path_buf(),
            run_counter,
            results: log.results,
            best_metric,
        })
    }

    pub fn record_result(
        &mut self,
        commit: &str,
        metric: f64,
        status: ResultStatus,
        description: &str,
        timestamp: DateTime<Utc>,
    ) -> std::io::Result<RecordOutcome> {
        self.run_counter = self.run_counter.saturating_add(1);

        let result = ExperimentResult {
            record_type: "result".to_string(),
            run: self.run_counter,
            commit: commit.to_string(),
            metric,
            metrics: None,
            status,
            description: description.to_string(),
            asi: None,
            timestamp,
        };

        jsonl::append_result(&self.log_path, &result)?;
        self.results.push(result);

        let is_minimize = self.config.is_minimize();

        // Compute confidence from kept results
        let kept_metrics: Vec<f64> =
            self.results.iter().filter(|r| r.status == ResultStatus::Keep).map(|r| r.metric).collect();

        let conf = confidence::compute_confidence(&kept_metrics, metric, is_minimize);

        // Update best
        let is_new_best = match (self.best_metric, status) {
            (None, ResultStatus::Keep) => true,
            (Some(best), ResultStatus::Keep) => {
                if is_minimize {
                    metric < best
                } else {
                    metric > best
                }
            }
            _ => false,
        };

        if is_new_best {
            self.best_metric = Some(metric);
        }

        // Git operations
        match status {
            ResultStatus::Keep => {
                let msg = format!(
                    "autoresearch: run {} {} ({}={})",
                    self.run_counter, description, self.config.metric_name, metric
                );
                if let Err(e) = crate::git::commit(&self.cwd, &msg) {
                    tracing::warn!("autoresearch git commit failed: {e}");
                }
            }
            ResultStatus::Discard | ResultStatus::Crash | ResultStatus::ChecksFailed => {
                if let Err(e) = crate::git::revert_preserving(&self.cwd, &["autoresearch.jsonl"]) {
                    tracing::warn!("autoresearch git revert failed: {e}");
                }
            }
        }

        Ok(RecordOutcome {
            run: self.run_counter,
            is_new_best,
            best_metric: self.best_metric,
            confidence: conf,
        })
    }

    pub fn kept_count(&self) -> u64 {
        self.results.iter().filter(|r| r.status == ResultStatus::Keep).count() as u64
    }

    pub fn total_runs(&self) -> u32 {
        self.run_counter
    }
}

#[derive(Debug)]
pub struct RecordOutcome {
    pub run: u32,
    pub is_new_best: bool,
    pub best_metric: Option<f64>,
    pub confidence: Option<confidence::ConfidenceResult>,
}

fn compute_best(results: &[ExperimentResult], is_minimize: bool) -> Option<f64> {
    results
        .iter()
        .filter(|r| r.status == ResultStatus::Keep)
        .map(|r| r.metric)
        .reduce(|a, b| if is_minimize { a.min(b) } else { a.max(b) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_timestamp() -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000, 0).expect("valid test timestamp")
    }

    #[test]
    fn init_creates_log() {
        let tmp = tempfile::TempDir::new().unwrap();
        let session = ExperimentSession::init(ExperimentInitOptions {
            cwd: tmp.path(),
            name: "test",
            metric_name: "latency",
            metric_unit: Some("ms"),
            direction: Some("minimize"),
            timestamp: test_timestamp(),
        })
        .unwrap();
        assert_eq!(session.run_counter, 0);
        assert!(session.log_path.exists());
    }

    #[test]
    fn lifecycle_init_keep_discard_resume() {
        let tmp = tempfile::TempDir::new().unwrap();

        // Init git repo for git ops
        std::process::Command::new("git").args(["init"]).current_dir(tmp.path()).status().unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(tmp.path())
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(tmp.path())
            .status()
            .unwrap();
        std::fs::write(tmp.path().join("code.rs"), "fn main() {}").unwrap();
        std::process::Command::new("git").args(["add", "-A"]).current_dir(tmp.path()).status().unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(tmp.path())
            .status()
            .unwrap();

        let mut session = ExperimentSession::init(ExperimentInitOptions {
            cwd: tmp.path(),
            name: "test",
            metric_name: "score",
            metric_unit: None,
            direction: None,
            timestamp: test_timestamp(),
        })
        .unwrap();

        // Run 1: keep
        let outcome = session
            .record_result("abc1234", 10.0, ResultStatus::Keep, "first try", test_timestamp())
            .unwrap();
        assert_eq!(outcome.run, 1);
        assert!(outcome.is_new_best);

        // Run 2: keep (better)
        let outcome = session
            .record_result("def5678", 15.0, ResultStatus::Keep, "second try", test_timestamp())
            .unwrap();
        assert_eq!(outcome.run, 2);
        assert!(outcome.is_new_best);
        assert!((outcome.best_metric.unwrap() - 15.0).abs() < f64::EPSILON);

        // Run 3: discard
        let outcome = session
            .record_result("ghi9012", 5.0, ResultStatus::Discard, "bad try", test_timestamp())
            .unwrap();
        assert_eq!(outcome.run, 3);
        assert!(!outcome.is_new_best);

        // Resume from existing log
        let resumed = ExperimentSession::init(ExperimentInitOptions {
            cwd: tmp.path(),
            name: "test-v2",
            metric_name: "score",
            metric_unit: None,
            direction: None,
            timestamp: test_timestamp(),
        })
        .unwrap();
        assert_eq!(resumed.run_counter, 3);
        assert_eq!(resumed.results.len(), 3);
    }
}
