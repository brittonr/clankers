//! Fixed-size metrics data structures.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::collections::BTreeMap;

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

// ── Latency histogram ───────────────────────────────────────────────

const LATENCY_BUCKETS: [u64; 12] = [1, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000];

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LatencyHistogram {
    buckets: [u64; 12],
    over: u64,
    count: u64,
    sum_ms: u64,
}

impl LatencyHistogram {
    pub fn record(&mut self, ms: u64) {
        self.count += 1;
        self.sum_ms += ms;
        for (i, &bound) in LATENCY_BUCKETS.iter().enumerate() {
            if ms <= bound {
                self.buckets[i] += 1;
                return;
            }
        }
        self.over += 1;
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    pub fn sum_ms(&self) -> u64 {
        self.sum_ms
    }

    pub fn mean_ms(&self) -> Option<f64> {
        if self.count == 0 {
            return None;
        }
        Some(self.sum_ms as f64 / self.count as f64)
    }

    pub fn merge(&mut self, other: &Self) {
        for (i, v) in other.buckets.iter().enumerate() {
            self.buckets[i] += v;
        }
        self.over += other.over;
        self.count += other.count;
        self.sum_ms += other.sum_ms;
    }
}

// ── Bounded top-K counter ───────────────────────────────────────────

const DEFAULT_TOP_K: usize = 20;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TopKCounter {
    entries: BTreeMap<String, u64>,
    other_count: u64,
    cap: usize,
}

impl TopKCounter {
    pub fn new(cap: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            other_count: 0,
            cap,
        }
    }

    pub fn increment(&mut self, key: &str) {
        if let Some(v) = self.entries.get_mut(key) {
            *v += 1;
            return;
        }
        if self.entries.len() < self.cap {
            self.entries.insert(key.to_string(), 1);
            return;
        }
        // Evict the lowest entry if the new key would surpass it.
        let Some((evict_key, min_count)) = self.lowest_entry() else {
            self.other_count = self.other_count.saturating_add(1);
            return;
        };
        if 1 > min_count {
            self.other_count = self.other_count.saturating_add(min_count);
            self.entries.remove(&evict_key);
            self.entries.insert(key.to_string(), 1);
        } else {
            self.other_count = self.other_count.saturating_add(1);
        }
    }

    pub fn top(&self) -> &BTreeMap<String, u64> {
        &self.entries
    }

    pub fn other_count(&self) -> u64 {
        self.other_count
    }

    pub fn total(&self) -> u64 {
        let tracked: u64 = self.entries.values().sum();
        tracked.saturating_add(self.other_count)
    }

    pub fn merge(&mut self, other: &Self) {
        for (k, &v) in &other.entries {
            *self.entries.entry(k.clone()).or_insert(0) += v;
        }
        self.other_count += other.other_count;
        // Re-cap after merge by evicting lowest until within cap.
        while self.entries.len() > self.cap {
            let Some((evict_key, min_count)) = self.lowest_entry() else {
                return;
            };
            self.other_count = self.other_count.saturating_add(min_count);
            self.entries.remove(&evict_key);
        }
    }

    fn lowest_entry(&self) -> Option<(String, u64)> {
        self.entries
            .iter()
            .min_by(|left, right| left.1.cmp(right.1).then_with(|| left.0.cmp(right.0)))
            .map(|(key, count)| (key.clone(), *count))
    }
}

// ── Session metrics summary ─────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SessionMetricsSummary {
    pub session_id: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,

    // Turn counts
    pub turns_total: u32,
    pub turns_cancelled: u32,

    // Token usage
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,

    // Compaction
    pub compactions: u32,
    pub compaction_tokens_saved: u64,

    // Model usage
    pub model_switches: u32,
    pub models: TopKCounter,

    // Tool latency and counts
    pub tool_latency: LatencyHistogram,
    pub tools: TopKCounter,
    pub tool_errors: u32,

    // Plugin activity
    pub plugin_loads: TopKCounter,
    pub plugin_events: u32,
    pub plugin_errors: u32,
    pub plugin_hook_denials: u32,

    // Process monitoring aggregates
    pub procmon_spawns: u32,
    pub procmon_peak_rss: u64,

    // Recent events tracking
    pub recent_events_stored: u32,
    pub recent_events_dropped: u32,
}

impl SessionMetricsSummary {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            models: TopKCounter::new(DEFAULT_TOP_K),
            tools: TopKCounter::new(DEFAULT_TOP_K),
            plugin_loads: TopKCounter::new(DEFAULT_TOP_K),
            ..Default::default()
        }
    }

    pub fn duration_secs(&self) -> Option<f64> {
        match (self.started_at, self.ended_at) {
            (Some(s), Some(e)) => {
                let dur = e.signed_duration_since(s);
                Some(dur.num_milliseconds() as f64 / 1000.0)
            }
            _ => None,
        }
    }

    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

// ── Daily rollup ────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DailyMetricsRollup {
    pub date: String,
    pub sessions: u32,
    pub turns: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub tool_calls: u64,
    pub tool_errors: u32,
    pub compactions: u32,
    pub models: TopKCounter,
    pub tools: TopKCounter,
}

impl DailyMetricsRollup {
    pub fn new(date: String) -> Self {
        Self {
            date,
            models: TopKCounter::new(DEFAULT_TOP_K),
            tools: TopKCounter::new(DEFAULT_TOP_K),
            ..Default::default()
        }
    }

    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }

    pub fn merge_session(&mut self, summary: &SessionMetricsSummary) {
        self.sessions += 1;
        self.turns += summary.turns_total;
        self.input_tokens += summary.input_tokens;
        self.output_tokens += summary.output_tokens;
        self.tool_calls += summary.tool_latency.count();
        self.tool_errors += summary.tool_errors;
        self.compactions += summary.compactions;
        self.models.merge(&summary.models);
        self.tools.merge(&summary.tools);
    }
}

// ── Recent metric event ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricEventRecord {
    pub session_id: String,
    pub seq: u32,
    pub timestamp: DateTime<Utc>,
    pub kind: MetricEventKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MetricEventKind {
    SessionStart,
    SessionEnd,
    TurnStart {
        index: u32,
    },
    TurnEnd {
        index: u32,
        tool_calls: u32,
    },
    TurnCancel,
    ModelChange {
        from: String,
        to: String,
    },
    Compaction {
        tokens_saved: usize,
    },
    ToolExec {
        tool: String,
        duration_ms: u64,
        is_error: bool,
    },
    PluginLoad {
        plugin: String,
        ok: bool,
    },
    PluginEvent {
        plugin: String,
    },
    PluginError {
        plugin: String,
    },
    PluginHookDenial {
        plugin: String,
        hook: String,
    },
    UsageUpdate {
        input_tokens: u64,
        output_tokens: u64,
        model: String,
    },
    ProcessSpawn {
        pid: u32,
    },
    ProcessExit {
        pid: u32,
        peak_rss: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn histogram_record_and_mean() {
        let mut h = LatencyHistogram::default();
        h.record(10);
        h.record(20);
        h.record(30);
        assert_eq!(h.count(), 3);
        assert_eq!(h.sum_ms(), 60);
        assert!((h.mean_ms().unwrap() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn histogram_overflow_bucket() {
        let mut h = LatencyHistogram::default();
        h.record(99999);
        assert_eq!(h.count(), 1);
        assert_eq!(h.over, 1);
    }

    #[test]
    fn histogram_merge() {
        let mut a = LatencyHistogram::default();
        a.record(5);
        let mut b = LatencyHistogram::default();
        b.record(100);
        a.merge(&b);
        assert_eq!(a.count(), 2);
        assert_eq!(a.sum_ms(), 105);
    }

    #[test]
    fn histogram_empty_mean() {
        let h = LatencyHistogram::default();
        assert!(h.mean_ms().is_none());
    }

    #[test]
    fn topk_basic_increment() {
        let mut t = TopKCounter::new(3);
        t.increment("a");
        t.increment("b");
        t.increment("a");
        assert_eq!(*t.top().get("a").unwrap(), 2);
        assert_eq!(*t.top().get("b").unwrap(), 1);
        assert_eq!(t.total(), 3);
        assert_eq!(t.other_count(), 0);
    }

    #[test]
    fn topk_eviction_at_cap() {
        let mut t = TopKCounter::new(2);
        t.increment("a");
        t.increment("b");
        // Both at count 1. Adding "c" (count 1) can't beat min (1), goes to other.
        t.increment("c");
        assert_eq!(t.entries.len(), 2);
        assert_eq!(t.other_count(), 1);
        assert_eq!(t.total(), 3);
    }

    #[test]
    fn topk_zero_capacity_counts_everything_as_other() {
        let mut t = TopKCounter::new(0);
        t.increment("a");
        t.increment("b");
        assert!(t.entries.is_empty());
        assert_eq!(t.other_count(), 2);
        assert_eq!(t.total(), 2);
    }

    #[test]
    fn topk_merge_recaps() {
        let mut a = TopKCounter::new(2);
        a.increment("x");
        a.increment("y");
        let mut b = TopKCounter::new(2);
        b.increment("y");
        b.increment("z");
        a.merge(&b);
        // After merge, cap is 2. y=2 should survive, one of x/z evicted.
        assert_eq!(a.entries.len(), 2);
        assert!(a.entries.contains_key("y"));
        assert_eq!(*a.entries.get("y").unwrap(), 2);
    }

    #[test]
    fn session_summary_duration() {
        let mut s = SessionMetricsSummary::new("test".into());
        s.started_at = Some(DateTime::parse_from_rfc3339("2026-04-24T10:00:00Z").unwrap().to_utc());
        s.ended_at = Some(DateTime::parse_from_rfc3339("2026-04-24T10:05:30Z").unwrap().to_utc());
        assert!((s.duration_secs().unwrap() - 330.0).abs() < 0.01);
    }

    #[test]
    fn session_summary_no_duration_without_end() {
        let s = SessionMetricsSummary::new("test".into());
        assert!(s.duration_secs().is_none());
    }

    #[test]
    fn daily_rollup_merge_session() {
        let mut rollup = DailyMetricsRollup::new("2026-04-24".into());
        let mut s = SessionMetricsSummary::new("s1".into());
        s.turns_total = 5;
        s.input_tokens = 1000;
        s.output_tokens = 500;
        s.tool_errors = 1;
        s.compactions = 2;
        s.tool_latency.record(50);
        s.tool_latency.record(100);
        rollup.merge_session(&s);

        assert_eq!(rollup.sessions, 1);
        assert_eq!(rollup.turns, 5);
        assert_eq!(rollup.input_tokens, 1000);
        assert_eq!(rollup.output_tokens, 500);
        assert_eq!(rollup.tool_calls, 2);
        assert_eq!(rollup.tool_errors, 1);
        assert_eq!(rollup.compactions, 2);
    }

    #[test]
    fn metric_event_record_serde_roundtrip() {
        let record = MetricEventRecord {
            session_id: "abc".into(),
            seq: 42,
            timestamp: Utc::now(),
            kind: MetricEventKind::ToolExec {
                tool: "bash".into(),
                duration_ms: 150,
                is_error: false,
            },
        };
        let json = serde_json::to_string(&record).unwrap();
        let back: MetricEventRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back.session_id, "abc");
        assert_eq!(back.seq, 42);
    }
}
