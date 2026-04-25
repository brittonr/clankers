//! Metric extraction from command output.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

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
