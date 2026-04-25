//! Metric extraction from command output.

use std::collections::HashMap;

pub fn extract_metrics(output: &str) -> HashMap<String, f64> {
    let mut metrics = HashMap::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("METRIC ") {
            if let Some((name, value_str)) = rest.split_once('=') {
                let name = name.trim();
                let value_str = value_str.trim();
                if let Ok(v) = value_str.parse::<f64>() {
                    metrics.insert(name.to_string(), v);
                }
            }
        }
    }
    metrics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_single_metric() {
        let output = "Building...\nMETRIC latency=42.5\nDone.";
        let m = extract_metrics(output);
        assert_eq!(m.len(), 1);
        assert!((m["latency"] - 42.5).abs() < f64::EPSILON);
    }

    #[test]
    fn extract_multiple_metrics() {
        let output = "METRIC latency=42.5\nMETRIC throughput=1000\nMETRIC accuracy=0.95";
        let m = extract_metrics(output);
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn scientific_notation() {
        let output = "METRIC loss=1.23e-4";
        let m = extract_metrics(output);
        assert!((m["loss"] - 1.23e-4).abs() < 1e-10);
    }

    #[test]
    fn no_metrics() {
        let m = extract_metrics("regular output\nno metrics here");
        assert!(m.is_empty());
    }

    #[test]
    fn malformed_lines_skipped() {
        let output = "METRIC =noname\nMETRIC badvalue=abc\nMETRIC good=1.0";
        let m = extract_metrics(output);
        assert_eq!(m.len(), 1);
        assert!(m.contains_key("good"));
    }

    #[test]
    fn integer_values() {
        let output = "METRIC count=42";
        let m = extract_metrics(output);
        assert!((m["count"] - 42.0).abs() < f64::EPSILON);
    }
}
