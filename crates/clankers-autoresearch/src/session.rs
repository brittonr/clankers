//! Experiment session manager — coordinates JSONL state, metrics, and git.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::path::Path;
use std::path::PathBuf;

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

impl ExperimentSession {
    pub fn init(
        cwd: &Path,
        name: &str,
        metric_name: &str,
        metric_unit: Option<&str>,
        direction: Option<&str>,
    ) -> std::io::Result<Self> {
        let log_path = cwd.join("autoresearch.jsonl");

        let (run_counter, results, best_metric) = if log_path.exists() {
            let log = jsonl::read_log(&log_path)?;
            let run_counter = log.results.last().map(|r| r.run).unwrap_or(0);
            let best = compute_best(&log.results, direction == Some("minimize"));
            (run_counter, log.results, best)
        } else {
            (0, Vec::new(), None)
        };

        let mut config = ExperimentConfig::new(name, metric_name);
        config.metric_unit = metric_unit.map(String::from);
        config.direction = direction.map(String::from);

        jsonl::append_config(&log_path, &config)?;

        Ok(Self {
            config,
            log_path,
            cwd: cwd.to_path_buf(),
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
    ) -> std::io::Result<RecordOutcome> {
        self.run_counter += 1;

        let result = ExperimentResult {
            record_type: "result".to_string(),
            run: self.run_counter,
            commit: commit.to_string(),
            metric,
            metrics: None,
            status,
            description: description.to_string(),
            asi: None,
            timestamp: Utc::now(),
        };

        jsonl::append_result(&self.log_path, &result)?;
        self.results.push(result);

        let minimize = self.config.is_minimize();

        // Compute confidence from kept results
        let kept_metrics: Vec<f64> =
            self.results.iter().filter(|r| r.status == ResultStatus::Keep).map(|r| r.metric).collect();

        let conf = confidence::compute_confidence(&kept_metrics, metric, minimize);

        // Update best
        let is_new_best = match (self.best_metric, status) {
            (None, ResultStatus::Keep) => true,
            (Some(best), ResultStatus::Keep) => {
                if minimize {
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

    pub fn kept_count(&self) -> usize {
        self.results.iter().filter(|r| r.status == ResultStatus::Keep).count()
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

fn compute_best(results: &[ExperimentResult], minimize: bool) -> Option<f64> {
    results
        .iter()
        .filter(|r| r.status == ResultStatus::Keep)
        .map(|r| r.metric)
        .reduce(|a, b| if minimize { a.min(b) } else { a.max(b) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_log() {
        let tmp = tempfile::TempDir::new().unwrap();
        let session = ExperimentSession::init(tmp.path(), "test", "latency", Some("ms"), Some("minimize")).unwrap();
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

        let mut session = ExperimentSession::init(tmp.path(), "test", "score", None, None).unwrap();

        // Run 1: keep
        let outcome = session.record_result("abc1234", 10.0, ResultStatus::Keep, "first try").unwrap();
        assert_eq!(outcome.run, 1);
        assert!(outcome.is_new_best);

        // Run 2: keep (better)
        let outcome = session.record_result("def5678", 15.0, ResultStatus::Keep, "second try").unwrap();
        assert_eq!(outcome.run, 2);
        assert!(outcome.is_new_best);
        assert!((outcome.best_metric.unwrap() - 15.0).abs() < f64::EPSILON);

        // Run 3: discard
        let outcome = session.record_result("ghi9012", 5.0, ResultStatus::Discard, "bad try").unwrap();
        assert_eq!(outcome.run, 3);
        assert!(!outcome.is_new_best);

        // Resume from existing log
        let resumed = ExperimentSession::init(tmp.path(), "test-v2", "score", None, None).unwrap();
        assert_eq!(resumed.run_counter, 3);
        assert_eq!(resumed.results.len(), 3);
    }
}
