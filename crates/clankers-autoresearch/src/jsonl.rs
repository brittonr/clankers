//! JSONL experiment log persistence.

use std::collections::HashMap;
use std::io::BufRead;
use std::path::Path;

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentConfig {
    #[serde(rename = "type")]
    pub record_type: String,
    pub name: String,
    pub metric_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl ExperimentConfig {
    pub fn new(name: &str, metric_name: &str) -> Self {
        Self {
            record_type: "config".to_string(),
            name: name.to_string(),
            metric_name: metric_name.to_string(),
            metric_unit: None,
            direction: None,
            timestamp: Utc::now(),
        }
    }

    pub fn is_minimize(&self) -> bool {
        self.direction.as_deref() == Some("minimize")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultStatus {
    Keep,
    Discard,
    Crash,
    ChecksFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentResult {
    #[serde(rename = "type")]
    pub record_type: String,
    pub run: u32,
    pub commit: String,
    pub metric: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<HashMap<String, f64>>,
    pub status: ResultStatus,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asi: Option<HashMap<String, serde_json::Value>>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ExperimentLog {
    pub config: Option<ExperimentConfig>,
    pub results: Vec<ExperimentResult>,
}

pub fn read_log(path: &Path) -> std::io::Result<ExperimentLog> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let mut config = None;
    let mut results = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        match value.get("type").and_then(|t| t.as_str()) {
            Some("config") => {
                if let Ok(c) = serde_json::from_value::<ExperimentConfig>(value) {
                    config = Some(c);
                }
            }
            Some("result") => {
                if let Ok(r) = serde_json::from_value::<ExperimentResult>(value) {
                    results.push(r);
                }
            }
            _ => {}
        }
    }

    Ok(ExperimentLog { config, results })
}

pub fn append_config(path: &Path, config: &ExperimentConfig) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    let json = serde_json::to_string(config).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    writeln!(file, "{json}")
}

pub fn append_result(path: &Path, result: &ExperimentResult) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    let json = serde_json::to_string(result).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    writeln!(file, "{json}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("autoresearch.jsonl");

        let config = ExperimentConfig::new("test-experiment", "latency_ms");
        append_config(&path, &config).unwrap();

        let log = read_log(&path).unwrap();
        assert!(log.config.is_some());
        assert_eq!(log.config.unwrap().metric_name, "latency_ms");
        assert!(log.results.is_empty());
    }

    #[test]
    fn result_round_trip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("autoresearch.jsonl");

        let config = ExperimentConfig::new("test", "val_bpb");
        append_config(&path, &config).unwrap();

        let result = ExperimentResult {
            record_type: "result".to_string(),
            run: 1,
            commit: "abc1234".to_string(),
            metric: 3.14,
            metrics: None,
            status: ResultStatus::Keep,
            description: "first run".to_string(),
            asi: None,
            timestamp: Utc::now(),
        };
        append_result(&path, &result).unwrap();

        let log = read_log(&path).unwrap();
        assert_eq!(log.results.len(), 1);
        assert_eq!(log.results[0].run, 1);
        assert!((log.results[0].metric - 3.14).abs() < f64::EPSILON);
    }

    #[test]
    fn resume_preserves_results() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("autoresearch.jsonl");

        let config = ExperimentConfig::new("test", "ms");
        append_config(&path, &config).unwrap();

        for i in 1..=3 {
            let result = ExperimentResult {
                record_type: "result".to_string(),
                run: i,
                commit: format!("hash{i}"),
                metric: f64::from(i) * 10.0,
                metrics: None,
                status: ResultStatus::Keep,
                description: format!("run {i}"),
                asi: None,
                timestamp: Utc::now(),
            };
            append_result(&path, &result).unwrap();
        }

        // Re-init by appending new config
        let config2 = ExperimentConfig::new("test-v2", "ms");
        append_config(&path, &config2).unwrap();

        let log = read_log(&path).unwrap();
        assert_eq!(log.config.unwrap().name, "test-v2");
        assert_eq!(log.results.len(), 3);
    }
}
