//! Translates AgentEvent into MetricEvent and drives the reducer.

use std::collections::HashMap;
use std::time::Instant;

use chrono::Utc;
use clankers_agent::events::AgentEvent;
use clankers_db::metrics::reducer::MetricEvent;
use clankers_db::metrics::reducer::MetricsReducer;
use clankers_db::metrics::types::MetricEventRecord;
use clankers_db::metrics::types::SessionMetricsSummary;

pub struct MetricsCollector {
    reducer: MetricsReducer,
    pending_events: Vec<MetricEventRecord>,
    tool_starts: HashMap<String, (String, Instant)>,
    current_model: String,
}

impl MetricsCollector {
    pub fn new(session_id: String) -> Self {
        Self {
            reducer: MetricsReducer::new(session_id),
            pending_events: Vec::new(),
            tool_starts: HashMap::new(),
            current_model: String::new(),
        }
    }

    pub fn set_model(&mut self, model: String) {
        self.current_model = model;
    }

    pub fn process(&mut self, event: &AgentEvent) {
        let metric_events = self.translate(event);
        for me in metric_events {
            let record = self.reducer.apply(&me);
            self.pending_events.push(record);
        }
    }

    pub fn take_pending(&mut self) -> Vec<MetricEventRecord> {
        std::mem::take(&mut self.pending_events)
    }

    pub fn summary(&self) -> &SessionMetricsSummary {
        self.reducer.summary()
    }

    pub fn into_summary(self) -> SessionMetricsSummary {
        self.reducer.into_summary()
    }

    fn translate(&mut self, event: &AgentEvent) -> Vec<MetricEvent> {
        let now = Utc::now();
        match event {
            AgentEvent::SessionStart { session_id } => vec![MetricEvent::SessionStart {
                session_id: session_id.clone(),
                timestamp: now,
            }],
            AgentEvent::SessionShutdown { session_id } => vec![MetricEvent::SessionEnd {
                session_id: session_id.clone(),
                timestamp: now,
            }],
            AgentEvent::TurnStart { index } => vec![MetricEvent::TurnStart {
                index: *index,
                timestamp: now,
            }],
            AgentEvent::TurnEnd { index, .. } => vec![MetricEvent::TurnEnd {
                index: *index,
                tool_calls: 0,
                timestamp: now,
            }],
            AgentEvent::UserCancel => vec![MetricEvent::TurnCancel { timestamp: now }],
            AgentEvent::ModelChange { from, to, .. } => vec![MetricEvent::ModelChange {
                from: from.clone(),
                to: to.clone(),
                timestamp: now,
            }],
            AgentEvent::SessionCompaction { tokens_saved, .. } => vec![MetricEvent::Compaction {
                tokens_saved: *tokens_saved,
                timestamp: now,
            }],
            AgentEvent::ToolExecutionStart { call_id, tool_name } => {
                self.tool_starts
                    .insert(call_id.clone(), (tool_name.clone(), Instant::now()));
                vec![]
            }
            AgentEvent::ToolExecutionEnd {
                call_id, is_error, ..
            } => {
                let (tool_name, duration_ms) = match self.tool_starts.remove(call_id) {
                    Some((name, start)) => (name, start.elapsed().as_millis() as u64),
                    None => ("unknown".to_string(), 0),
                };
                vec![MetricEvent::ToolExec {
                    tool: tool_name,
                    duration_ms,
                    is_error: *is_error,
                    timestamp: now,
                }]
            }
            AgentEvent::UsageUpdate {
                turn_usage,
                cumulative_usage: _,
            } => vec![MetricEvent::UsageUpdate {
                model: self.current_model.clone(),
                input_tokens: turn_usage.input_tokens as u64,
                output_tokens: turn_usage.output_tokens as u64,
                cache_creation_tokens: turn_usage.cache_creation_input_tokens as u64,
                cache_read_tokens: turn_usage.cache_read_input_tokens as u64,
                timestamp: now,
            }],
            AgentEvent::ProcessSpawn { pid, .. } => vec![MetricEvent::ProcessSpawn {
                pid: *pid,
                timestamp: now,
            }],
            AgentEvent::ProcessExit { pid, peak_rss, .. } => vec![MetricEvent::ProcessExit {
                pid: *pid,
                peak_rss: *peak_rss,
                timestamp: now,
            }],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use clankers_agent::ToolResult;

    use super::*;

    fn test_assistant_message() -> clankers_provider::message::AssistantMessage {
        clankers_provider::message::AssistantMessage {
            id: clankers_provider::message::MessageId::new("a1"),
            content: vec![],
            model: "test".to_string(),
            usage: clankers_provider::Usage::default(),
            stop_reason: clankers_provider::message::StopReason::Stop,
            timestamp: chrono::Utc::now(),
        }
    }

    #[test]
    fn session_lifecycle_captured() {
        let mut c = MetricsCollector::new("s1".into());
        c.process(&AgentEvent::SessionStart {
            session_id: "s1".into(),
        });
        c.process(&AgentEvent::TurnStart { index: 0 });
        c.process(&AgentEvent::TurnEnd {
            index: 0,
            message: test_assistant_message(),
            tool_results: vec![],
        });
        c.process(&AgentEvent::SessionShutdown {
            session_id: "s1".into(),
        });

        assert_eq!(c.summary().turns_total, 1);
        assert!(c.summary().started_at.is_some());
        assert!(c.summary().ended_at.is_some());
        let pending = c.take_pending();
        assert_eq!(pending.len(), 4);
    }

    #[test]
    fn tool_exec_captured_with_timing() {
        let mut c = MetricsCollector::new("s1".into());
        c.process(&AgentEvent::ToolExecutionStart {
            call_id: "c1".into(),
            tool_name: "bash".into(),
        });
        std::thread::sleep(std::time::Duration::from_millis(5));
        c.process(&AgentEvent::ToolExecutionEnd {
            call_id: "c1".into(),
            result: ToolResult::text("ok"),
            is_error: false,
        });
        assert_eq!(c.summary().tool_latency.count(), 1);
        assert!(c.summary().tool_latency.sum_ms() > 0);
    }

    #[test]
    fn usage_captured() {
        let mut c = MetricsCollector::new("s1".into());
        c.set_model("sonnet".into());
        let usage = clankers_provider::Usage {
            input_tokens: 500,
            output_tokens: 200,
            cache_creation_input_tokens: 10,
            cache_read_input_tokens: 5,
        };
        c.process(&AgentEvent::UsageUpdate {
            turn_usage: usage.clone(),
            cumulative_usage: usage,
        });
        assert_eq!(c.summary().input_tokens, 500);
        assert_eq!(c.summary().output_tokens, 200);
    }

    #[test]
    fn model_change_captured() {
        let mut c = MetricsCollector::new("s1".into());
        c.process(&AgentEvent::ModelChange {
            from: "sonnet".into(),
            to: "opus".into(),
            reason: "user request".into(),
        });
        assert_eq!(c.summary().model_switches, 1);
    }

    #[test]
    fn unhandled_events_produce_no_metrics() {
        let mut c = MetricsCollector::new("s1".into());
        c.process(&AgentEvent::AgentStart);
        assert!(c.take_pending().is_empty());
    }
}
