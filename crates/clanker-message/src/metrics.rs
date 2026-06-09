//! Neutral metrics contracts and reducer for agent/controller sessions.
//!
//! This module is intentionally storage-free. Database crates may persist these
//! records, while controller/runtime crates can reduce events without depending
//! on a concrete database implementation.

use std::collections::BTreeMap;

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

const LATENCY_BUCKET_MS_BOUNDS: [u64; 12] = [1, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000];
const DEFAULT_TOP_COUNTER_CAPACITY_COUNT: u32 = 20;
const _: () = assert!(usize::BITS >= u32::BITS);
const _: () = assert!(usize::BITS <= u64::BITS);

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LatencyHistogram {
    buckets: [u64; 12],
    over: u64,
    count: u64,
    sum_ms: u64,
}

impl LatencyHistogram {
    pub fn record(&mut self, duration_ms: u64) {
        self.count = self.count.saturating_add(1);
        self.sum_ms = self.sum_ms.saturating_add(duration_ms);
        for (bucket_index, &bound_ms) in LATENCY_BUCKET_MS_BOUNDS.iter().enumerate() {
            if duration_ms <= bound_ms {
                self.buckets[bucket_index] = self.buckets[bucket_index].saturating_add(1);
                return;
            }
        }
        self.over = self.over.saturating_add(1);
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
        for (bucket_index, value) in other.buckets.iter().enumerate() {
            self.buckets[bucket_index] = self.buckets[bucket_index].saturating_add(*value);
        }
        self.over = self.over.saturating_add(other.over);
        self.count = self.count.saturating_add(other.count);
        self.sum_ms = self.sum_ms.saturating_add(other.sum_ms);
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TopCounter {
    entries: BTreeMap<String, u64>,
    other_count: u64,
    #[serde(rename = "cap")]
    capacity_count: u32,
}

impl TopCounter {
    pub fn new(capacity_count: u32) -> Self {
        Self {
            entries: BTreeMap::new(),
            other_count: 0,
            capacity_count,
        }
    }

    pub fn increment(&mut self, key: &str) {
        if let Some(value) = self.entries.get_mut(key) {
            *value = value.saturating_add(1);
            return;
        }
        if self.entry_count() < u64::from(self.capacity_count) {
            self.entries.insert(key.to_string(), 1);
            return;
        }
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
        for (key, &value) in &other.entries {
            let entry = self.entries.entry(key.clone()).or_insert(0);
            *entry = entry.saturating_add(value);
        }
        self.other_count = self.other_count.saturating_add(other.other_count);
        while self.entry_count() > u64::from(self.capacity_count) {
            let Some((evict_key, min_count)) = self.lowest_entry() else {
                return;
            };
            self.other_count = self.other_count.saturating_add(min_count);
            self.entries.remove(&evict_key);
        }
    }

    fn entry_count(&self) -> u64 {
        self.entries.len() as u64
    }

    fn lowest_entry(&self) -> Option<(String, u64)> {
        self.entries
            .iter()
            .min_by(|left, right| left.1.cmp(right.1).then_with(|| left.0.cmp(right.0)))
            .map(|(key, count)| (key.clone(), *count))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SessionMetricsSummary {
    pub session_id: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub turns_total: u32,
    pub turns_cancelled: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub compactions: u32,
    pub compaction_tokens_saved: u64,
    pub model_switches: u32,
    pub models: TopCounter,
    pub tool_latency: LatencyHistogram,
    pub tools: TopCounter,
    pub tool_errors: u32,
    pub plugin_loads: TopCounter,
    pub plugin_events: u32,
    pub plugin_errors: u32,
    pub plugin_hook_denials: u32,
    pub procmon_spawns: u32,
    pub procmon_peak_rss: u64,
    pub recent_events_stored: u32,
    pub recent_events_dropped: u32,
}

impl SessionMetricsSummary {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            models: TopCounter::new(DEFAULT_TOP_COUNTER_CAPACITY_COUNT),
            tools: TopCounter::new(DEFAULT_TOP_COUNTER_CAPACITY_COUNT),
            plugin_loads: TopCounter::new(DEFAULT_TOP_COUNTER_CAPACITY_COUNT),
            ..Default::default()
        }
    }

    pub fn duration_secs(&self) -> Option<f64> {
        match (self.started_at, self.ended_at) {
            (Some(started), Some(ended)) => {
                let duration_ms = ended.signed_duration_since(started).num_milliseconds();
                Some(duration_ms as f64 / 1000.0)
            }
            _ => None,
        }
    }

    pub fn total_tokens(&self) -> u64 {
        self.input_tokens.saturating_add(self.output_tokens)
    }
}

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
    pub models: TopCounter,
    pub tools: TopCounter,
}

impl DailyMetricsRollup {
    pub fn new(date: String) -> Self {
        Self {
            date,
            models: TopCounter::new(DEFAULT_TOP_COUNTER_CAPACITY_COUNT),
            tools: TopCounter::new(DEFAULT_TOP_COUNTER_CAPACITY_COUNT),
            ..Default::default()
        }
    }

    pub fn total_tokens(&self) -> u64 {
        self.input_tokens.saturating_add(self.output_tokens)
    }

    pub fn merge_session(&mut self, summary: &SessionMetricsSummary) {
        self.sessions = self.sessions.saturating_add(1);
        self.turns = self.turns.saturating_add(summary.turns_total);
        self.input_tokens = self.input_tokens.saturating_add(summary.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(summary.output_tokens);
        self.tool_calls = self.tool_calls.saturating_add(summary.tool_latency.count());
        self.tool_errors = self.tool_errors.saturating_add(summary.tool_errors);
        self.compactions = self.compactions.saturating_add(summary.compactions);
        self.models.merge(&summary.models);
        self.tools.merge(&summary.tools);
    }
}

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
    TurnStart { index: u32 },
    TurnEnd { index: u32, tool_calls: u32 },
    TurnCancel,
    ModelChange { from: String, to: String },
    Compaction { tokens_saved: usize },
    ToolExec { tool: String, duration_ms: u64, is_error: bool },
    PluginLoad { plugin: String, ok: bool },
    PluginEvent { plugin: String },
    PluginError { plugin: String },
    PluginHookDenial { plugin: String, hook: String },
    UsageUpdate { input_tokens: u64, output_tokens: u64, model: String },
    ProcessSpawn { pid: u32 },
    ProcessExit { pid: u32, peak_rss: u64 },
}

#[derive(Debug, Clone)]
pub enum MetricEvent {
    SessionStart { session_id: String, timestamp: DateTime<Utc> },
    SessionEnd { session_id: String, timestamp: DateTime<Utc> },
    TurnStart { index: u32, timestamp: DateTime<Utc> },
    TurnEnd { index: u32, tool_calls: u32, timestamp: DateTime<Utc> },
    TurnCancel { timestamp: DateTime<Utc> },
    ModelChange { from: String, to: String, timestamp: DateTime<Utc> },
    Compaction { tokens_saved: usize, timestamp: DateTime<Utc> },
    ToolExec { tool: String, duration_ms: u64, is_error: bool, timestamp: DateTime<Utc> },
    UsageUpdate {
        model: String,
        input_tokens: u64,
        output_tokens: u64,
        cache_creation_tokens: u64,
        cache_read_tokens: u64,
        timestamp: DateTime<Utc>,
    },
    PluginLoad { plugin: String, ok: bool, timestamp: DateTime<Utc> },
    PluginEvent { plugin: String, timestamp: DateTime<Utc> },
    PluginError { plugin: String, timestamp: DateTime<Utc> },
    PluginHookDenial { plugin: String, hook: String, timestamp: DateTime<Utc> },
    ProcessSpawn { pid: u32, timestamp: DateTime<Utc> },
    ProcessExit { pid: u32, peak_rss: u64, timestamp: DateTime<Utc> },
}

impl MetricEvent {
    fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::SessionStart { timestamp, .. }
            | Self::SessionEnd { timestamp, .. }
            | Self::TurnStart { timestamp, .. }
            | Self::TurnEnd { timestamp, .. }
            | Self::TurnCancel { timestamp }
            | Self::ModelChange { timestamp, .. }
            | Self::Compaction { timestamp, .. }
            | Self::ToolExec { timestamp, .. }
            | Self::UsageUpdate { timestamp, .. }
            | Self::PluginLoad { timestamp, .. }
            | Self::PluginEvent { timestamp, .. }
            | Self::PluginError { timestamp, .. }
            | Self::PluginHookDenial { timestamp, .. }
            | Self::ProcessSpawn { timestamp, .. }
            | Self::ProcessExit { timestamp, .. } => *timestamp,
        }
    }

    fn session_id_ref(&self) -> Option<&str> {
        match self {
            Self::SessionStart { session_id, .. } | Self::SessionEnd { session_id, .. } => Some(session_id),
            _ => None,
        }
    }
}

pub struct MetricsReducer {
    summary: SessionMetricsSummary,
    event_seq: u32,
}

impl MetricsReducer {
    pub fn new(session_id: String) -> Self {
        Self {
            summary: SessionMetricsSummary::new(session_id),
            event_seq: 0,
        }
    }

    pub fn apply(&mut self, event: &MetricEvent) -> MetricEventRecord {
        let seq = self.event_seq;
        self.event_seq = self.event_seq.saturating_add(1);

        let kind = match event {
            MetricEvent::SessionStart { .. } => {
                self.summary.started_at = Some(event.timestamp());
                MetricEventKind::SessionStart
            }
            MetricEvent::SessionEnd { .. } => {
                self.summary.ended_at = Some(event.timestamp());
                MetricEventKind::SessionEnd
            }
            MetricEvent::TurnStart { index, .. } => {
                self.summary.turns_total = self.summary.turns_total.saturating_add(1);
                MetricEventKind::TurnStart { index: *index }
            }
            MetricEvent::TurnEnd { index, tool_calls, .. } => MetricEventKind::TurnEnd {
                index: *index,
                tool_calls: *tool_calls,
            },
            MetricEvent::TurnCancel { .. } => {
                self.summary.turns_cancelled = self.summary.turns_cancelled.saturating_add(1);
                MetricEventKind::TurnCancel
            }
            MetricEvent::ModelChange { from, to, .. } => {
                self.summary.model_switches = self.summary.model_switches.saturating_add(1);
                self.summary.models.increment(to);
                MetricEventKind::ModelChange {
                    from: from.clone(),
                    to: to.clone(),
                }
            }
            MetricEvent::Compaction { tokens_saved, .. } => {
                let tokens_saved_count = *tokens_saved as u64;
                self.summary.compactions = self.summary.compactions.saturating_add(1);
                self.summary.compaction_tokens_saved = self
                    .summary
                    .compaction_tokens_saved
                    .saturating_add(tokens_saved_count);
                MetricEventKind::Compaction {
                    tokens_saved: *tokens_saved,
                }
            }
            MetricEvent::ToolExec {
                tool,
                duration_ms,
                is_error,
                ..
            } => {
                self.summary.tool_latency.record(*duration_ms);
                self.summary.tools.increment(tool);
                if *is_error {
                    self.summary.tool_errors = self.summary.tool_errors.saturating_add(1);
                }
                MetricEventKind::ToolExec {
                    tool: tool.clone(),
                    duration_ms: *duration_ms,
                    is_error: *is_error,
                }
            }
            MetricEvent::UsageUpdate {
                model,
                input_tokens,
                output_tokens,
                cache_creation_tokens,
                cache_read_tokens,
                ..
            } => {
                self.summary.input_tokens = self.summary.input_tokens.saturating_add(*input_tokens);
                self.summary.output_tokens = self.summary.output_tokens.saturating_add(*output_tokens);
                self.summary.cache_creation_tokens = self
                    .summary
                    .cache_creation_tokens
                    .saturating_add(*cache_creation_tokens);
                self.summary.cache_read_tokens = self
                    .summary
                    .cache_read_tokens
                    .saturating_add(*cache_read_tokens);
                self.summary.models.increment(model);
                MetricEventKind::UsageUpdate {
                    input_tokens: *input_tokens,
                    output_tokens: *output_tokens,
                    model: model.clone(),
                }
            }
            MetricEvent::PluginLoad { plugin, ok, .. } => {
                self.summary.plugin_loads.increment(plugin);
                if !ok {
                    self.summary.plugin_errors = self.summary.plugin_errors.saturating_add(1);
                }
                MetricEventKind::PluginLoad {
                    plugin: plugin.clone(),
                    ok: *ok,
                }
            }
            MetricEvent::PluginEvent { plugin, .. } => {
                self.summary.plugin_events = self.summary.plugin_events.saturating_add(1);
                MetricEventKind::PluginEvent { plugin: plugin.clone() }
            }
            MetricEvent::PluginError { plugin, .. } => {
                self.summary.plugin_errors = self.summary.plugin_errors.saturating_add(1);
                MetricEventKind::PluginError { plugin: plugin.clone() }
            }
            MetricEvent::PluginHookDenial { plugin, hook, .. } => {
                self.summary.plugin_hook_denials = self.summary.plugin_hook_denials.saturating_add(1);
                MetricEventKind::PluginHookDenial {
                    plugin: plugin.clone(),
                    hook: hook.clone(),
                }
            }
            MetricEvent::ProcessSpawn { pid, .. } => {
                self.summary.procmon_spawns = self.summary.procmon_spawns.saturating_add(1);
                MetricEventKind::ProcessSpawn { pid: *pid }
            }
            MetricEvent::ProcessExit { pid, peak_rss, .. } => {
                self.summary.procmon_peak_rss = self.summary.procmon_peak_rss.max(*peak_rss);
                MetricEventKind::ProcessExit {
                    pid: *pid,
                    peak_rss: *peak_rss,
                }
            }
        };

        MetricEventRecord {
            session_id: event.session_id_ref().unwrap_or(&self.summary.session_id).to_string(),
            seq,
            timestamp: event.timestamp(),
            kind,
        }
    }

    pub fn summary(&self) -> &SessionMetricsSummary {
        &self.summary
    }

    pub fn into_summary(self) -> SessionMetricsSummary {
        self.summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn histogram_record_and_mean() {
        let mut histogram = LatencyHistogram::default();
        histogram.record(10);
        histogram.record(20);
        histogram.record(30);
        assert_eq!(histogram.count(), 3);
        assert_eq!(histogram.sum_ms(), 60);
        assert!((histogram.mean_ms().expect("mean") - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn reducer_records_usage_and_tool_metrics() {
        let now = Utc::now();
        let mut reducer = MetricsReducer::new("session".to_string());
        reducer.apply(&MetricEvent::ToolExec {
            tool: "read".to_string(),
            duration_ms: 42,
            is_error: false,
            timestamp: now,
        });
        reducer.apply(&MetricEvent::UsageUpdate {
            model: "model".to_string(),
            input_tokens: 10,
            output_tokens: 5,
            cache_creation_tokens: 1,
            cache_read_tokens: 2,
            timestamp: now,
        });
        assert_eq!(reducer.summary().tool_latency.count(), 1);
        assert_eq!(reducer.summary().input_tokens, 10);
        assert_eq!(reducer.summary().output_tokens, 5);
    }
}
