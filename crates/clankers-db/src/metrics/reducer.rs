//! Pure metrics reducer — no I/O, no clock reads.

use chrono::DateTime;
use chrono::Utc;

use super::types::DailyMetricsRollup;
use super::types::MetricEventKind;
use super::types::MetricEventRecord;
use super::types::SessionMetricsSummary;

#[derive(Debug, Clone)]
pub enum MetricEvent {
    SessionStart {
        session_id: String,
        timestamp: DateTime<Utc>,
    },
    SessionEnd {
        session_id: String,
        timestamp: DateTime<Utc>,
    },
    TurnStart {
        index: u32,
        timestamp: DateTime<Utc>,
    },
    TurnEnd {
        index: u32,
        tool_calls: u32,
        timestamp: DateTime<Utc>,
    },
    TurnCancel {
        timestamp: DateTime<Utc>,
    },
    ModelChange {
        from: String,
        to: String,
        timestamp: DateTime<Utc>,
    },
    Compaction {
        tokens_saved: usize,
        timestamp: DateTime<Utc>,
    },
    ToolExec {
        tool: String,
        duration_ms: u64,
        is_error: bool,
        timestamp: DateTime<Utc>,
    },
    UsageUpdate {
        model: String,
        input_tokens: u64,
        output_tokens: u64,
        cache_creation_tokens: u64,
        cache_read_tokens: u64,
        timestamp: DateTime<Utc>,
    },
    PluginLoad {
        plugin: String,
        ok: bool,
        timestamp: DateTime<Utc>,
    },
    PluginEvent {
        plugin: String,
        timestamp: DateTime<Utc>,
    },
    PluginError {
        plugin: String,
        timestamp: DateTime<Utc>,
    },
    PluginHookDenial {
        plugin: String,
        hook: String,
        timestamp: DateTime<Utc>,
    },
    ProcessSpawn {
        pid: u32,
        timestamp: DateTime<Utc>,
    },
    ProcessExit {
        pid: u32,
        peak_rss: u64,
        timestamp: DateTime<Utc>,
    },
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
        self.event_seq += 1;

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
                self.summary.turns_total += 1;
                MetricEventKind::TurnStart { index: *index }
            }
            MetricEvent::TurnEnd { index, tool_calls, .. } => MetricEventKind::TurnEnd {
                index: *index,
                tool_calls: *tool_calls,
            },
            MetricEvent::TurnCancel { .. } => {
                self.summary.turns_cancelled += 1;
                MetricEventKind::TurnCancel
            }
            MetricEvent::ModelChange { from, to, .. } => {
                self.summary.model_switches += 1;
                self.summary.models.increment(to);
                MetricEventKind::ModelChange {
                    from: from.clone(),
                    to: to.clone(),
                }
            }
            MetricEvent::Compaction { tokens_saved, .. } => {
                self.summary.compactions += 1;
                self.summary.compaction_tokens_saved += *tokens_saved as u64;
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
                    self.summary.tool_errors += 1;
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
                self.summary.input_tokens += input_tokens;
                self.summary.output_tokens += output_tokens;
                self.summary.cache_creation_tokens += cache_creation_tokens;
                self.summary.cache_read_tokens += cache_read_tokens;
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
                    self.summary.plugin_errors += 1;
                }
                MetricEventKind::PluginLoad {
                    plugin: plugin.clone(),
                    ok: *ok,
                }
            }
            MetricEvent::PluginEvent { plugin, .. } => {
                self.summary.plugin_events += 1;
                MetricEventKind::PluginEvent { plugin: plugin.clone() }
            }
            MetricEvent::PluginError { plugin, .. } => {
                self.summary.plugin_errors += 1;
                MetricEventKind::PluginError { plugin: plugin.clone() }
            }
            MetricEvent::PluginHookDenial { plugin, hook, .. } => {
                self.summary.plugin_hook_denials += 1;
                MetricEventKind::PluginHookDenial {
                    plugin: plugin.clone(),
                    hook: hook.clone(),
                }
            }
            MetricEvent::ProcessSpawn { pid, .. } => {
                self.summary.procmon_spawns += 1;
                MetricEventKind::ProcessSpawn { pid: *pid }
            }
            MetricEvent::ProcessExit { pid, peak_rss, .. } => {
                if *peak_rss > self.summary.procmon_peak_rss {
                    self.summary.procmon_peak_rss = *peak_rss;
                }
                MetricEventKind::ProcessExit {
                    pid: *pid,
                    peak_rss: *peak_rss,
                }
            }
        };

        let session_id = event.session_id_ref().unwrap_or(&self.summary.session_id).to_string();

        MetricEventRecord {
            session_id,
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

    pub fn fold_into_rollup(&self, rollup: &mut DailyMetricsRollup) {
        rollup.merge_session(&self.summary);
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn ts(hour: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 4, 24, hour, min, 0).unwrap()
    }

    #[test]
    fn session_lifecycle() {
        let mut r = MetricsReducer::new("s1".into());
        r.apply(&MetricEvent::SessionStart {
            session_id: "s1".into(),
            timestamp: ts(10, 0),
        });
        r.apply(&MetricEvent::TurnStart {
            index: 0,
            timestamp: ts(10, 1),
        });
        r.apply(&MetricEvent::ToolExec {
            tool: "bash".into(),
            duration_ms: 200,
            is_error: false,
            timestamp: ts(10, 2),
        });
        r.apply(&MetricEvent::TurnEnd {
            index: 0,
            tool_calls: 1,
            timestamp: ts(10, 3),
        });
        r.apply(&MetricEvent::SessionEnd {
            session_id: "s1".into(),
            timestamp: ts(10, 5),
        });

        let s = r.summary();
        assert_eq!(s.turns_total, 1);
        assert_eq!(s.tool_latency.count(), 1);
        assert!((s.tool_latency.mean_ms().unwrap() - 200.0).abs() < 0.01);
        assert!(s.started_at.is_some());
        assert!(s.ended_at.is_some());
        assert!((s.duration_secs().unwrap() - 300.0).abs() < 0.01);
    }

    #[test]
    fn usage_accumulates() {
        let mut r = MetricsReducer::new("s1".into());
        r.apply(&MetricEvent::UsageUpdate {
            model: "sonnet".into(),
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_tokens: 10,
            cache_read_tokens: 5,
            timestamp: ts(10, 0),
        });
        r.apply(&MetricEvent::UsageUpdate {
            model: "sonnet".into(),
            input_tokens: 200,
            output_tokens: 100,
            cache_creation_tokens: 20,
            cache_read_tokens: 10,
            timestamp: ts(10, 1),
        });

        let s = r.summary();
        assert_eq!(s.input_tokens, 300);
        assert_eq!(s.output_tokens, 150);
        assert_eq!(s.cache_creation_tokens, 30);
        assert_eq!(s.cache_read_tokens, 15);
    }

    #[test]
    fn tool_errors_tracked() {
        let mut r = MetricsReducer::new("s1".into());
        r.apply(&MetricEvent::ToolExec {
            tool: "bash".into(),
            duration_ms: 50,
            is_error: true,
            timestamp: ts(10, 0),
        });
        assert_eq!(r.summary().tool_errors, 1);
    }

    #[test]
    fn model_switches() {
        let mut r = MetricsReducer::new("s1".into());
        r.apply(&MetricEvent::ModelChange {
            from: "sonnet".into(),
            to: "opus".into(),
            timestamp: ts(10, 0),
        });
        r.apply(&MetricEvent::ModelChange {
            from: "opus".into(),
            to: "haiku".into(),
            timestamp: ts(10, 1),
        });
        assert_eq!(r.summary().model_switches, 2);
    }

    #[test]
    fn cancellation_counted() {
        let mut r = MetricsReducer::new("s1".into());
        r.apply(&MetricEvent::TurnStart {
            index: 0,
            timestamp: ts(10, 0),
        });
        r.apply(&MetricEvent::TurnCancel { timestamp: ts(10, 1) });
        assert_eq!(r.summary().turns_total, 1);
        assert_eq!(r.summary().turns_cancelled, 1);
    }

    #[test]
    fn plugin_metrics() {
        let mut r = MetricsReducer::new("s1".into());
        r.apply(&MetricEvent::PluginLoad {
            plugin: "email".into(),
            ok: true,
            timestamp: ts(10, 0),
        });
        r.apply(&MetricEvent::PluginEvent {
            plugin: "email".into(),
            timestamp: ts(10, 1),
        });
        r.apply(&MetricEvent::PluginError {
            plugin: "email".into(),
            timestamp: ts(10, 2),
        });
        r.apply(&MetricEvent::PluginHookDenial {
            plugin: "email".into(),
            hook: "pre_exec".into(),
            timestamp: ts(10, 3),
        });
        let s = r.summary();
        assert_eq!(s.plugin_events, 1);
        assert_eq!(s.plugin_errors, 1);
        assert_eq!(s.plugin_hook_denials, 1);
    }

    #[test]
    fn procmon_peak_rss() {
        let mut r = MetricsReducer::new("s1".into());
        r.apply(&MetricEvent::ProcessSpawn {
            pid: 1234,
            timestamp: ts(10, 0),
        });
        r.apply(&MetricEvent::ProcessExit {
            pid: 1234,
            peak_rss: 100_000,
            timestamp: ts(10, 1),
        });
        r.apply(&MetricEvent::ProcessExit {
            pid: 5678,
            peak_rss: 50_000,
            timestamp: ts(10, 2),
        });
        assert_eq!(r.summary().procmon_spawns, 1);
        assert_eq!(r.summary().procmon_peak_rss, 100_000);
    }

    #[test]
    fn event_records_sequential_seqs() {
        let mut r = MetricsReducer::new("s1".into());
        let e1 = r.apply(&MetricEvent::TurnStart {
            index: 0,
            timestamp: ts(10, 0),
        });
        let e2 = r.apply(&MetricEvent::TurnEnd {
            index: 0,
            tool_calls: 0,
            timestamp: ts(10, 1),
        });
        assert_eq!(e1.seq, 0);
        assert_eq!(e2.seq, 1);
    }

    #[test]
    fn fold_into_rollup() {
        let mut r = MetricsReducer::new("s1".into());
        r.apply(&MetricEvent::TurnStart {
            index: 0,
            timestamp: ts(10, 0),
        });
        r.apply(&MetricEvent::UsageUpdate {
            model: "sonnet".into(),
            input_tokens: 1000,
            output_tokens: 500,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            timestamp: ts(10, 1),
        });
        let mut rollup = DailyMetricsRollup::new("2026-04-24".into());
        r.fold_into_rollup(&mut rollup);
        assert_eq!(rollup.sessions, 1);
        assert_eq!(rollup.turns, 1);
        assert_eq!(rollup.input_tokens, 1000);
    }
}
