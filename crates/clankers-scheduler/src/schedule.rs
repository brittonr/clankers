//! Schedule definition and lifecycle.

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

/// Unique schedule identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScheduleId(pub String);

impl ScheduleId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl std::fmt::Display for ScheduleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What kind of schedule this is.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScheduleKind {
    /// Fire once at a specific time.
    Once { at: DateTime<Utc> },
    /// Fire every `interval_secs` seconds.
    Interval { interval_secs: u64 },
    /// Fire on a cron-like pattern.
    Cron { pattern: crate::cron::CronPattern },
}

/// Current state of a schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleStatus {
    Active,
    Paused,
    /// Fired and auto-removed (only for `Once` schedules).
    Expired,
}

/// A scheduled task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub id: ScheduleId,
    pub name: String,
    pub kind: ScheduleKind,
    pub status: ScheduleStatus,
    /// Arbitrary payload — the consumer decides what this means.
    /// Typically `{"prompt": "..."}` or `{"tool": "bash", "params": {...}}`.
    pub payload: serde_json::Value,
    /// When this schedule was created.
    pub created_at: DateTime<Utc>,
    /// Last time this schedule fired (None if never).
    pub last_fired: Option<DateTime<Utc>>,
    /// How many times this schedule has fired.
    pub fire_count: u64,
    /// Max fires before auto-pause (None = unlimited).
    pub max_fires: Option<u64>,
}

impl Schedule {
    /// Create a new one-shot schedule.
    pub fn once(name: impl Into<String>, at: DateTime<Utc>, payload: serde_json::Value) -> Self {
        Self {
            id: ScheduleId::generate(),
            name: name.into(),
            kind: ScheduleKind::Once { at },
            status: ScheduleStatus::Active,
            payload,
            created_at: Utc::now(),
            last_fired: None,
            fire_count: 0,
            max_fires: Some(1),
        }
    }

    /// Create a new interval schedule.
    pub fn interval(name: impl Into<String>, interval_secs: u64, payload: serde_json::Value) -> Self {
        Self {
            id: ScheduleId::generate(),
            name: name.into(),
            kind: ScheduleKind::Interval { interval_secs },
            status: ScheduleStatus::Active,
            payload,
            created_at: Utc::now(),
            last_fired: None,
            fire_count: 0,
            max_fires: None,
        }
    }

    /// Create a new cron schedule.
    pub fn cron(name: impl Into<String>, pattern: crate::cron::CronPattern, payload: serde_json::Value) -> Self {
        Self {
            id: ScheduleId::generate(),
            name: name.into(),
            kind: ScheduleKind::Cron { pattern },
            status: ScheduleStatus::Active,
            payload,
            created_at: Utc::now(),
            last_fired: None,
            fire_count: 0,
            max_fires: None,
        }
    }

    /// Check whether this schedule should fire at `now`.
    pub fn should_fire(&self, now: DateTime<Utc>) -> bool {
        if self.status != ScheduleStatus::Active {
            return false;
        }
        if let Some(max) = self.max_fires
            && self.fire_count >= max
        {
            return false;
        }
        match &self.kind {
            ScheduleKind::Once { at } => now >= *at,
            ScheduleKind::Interval { interval_secs } => {
                let since = self.last_fired.unwrap_or(self.created_at);
                let elapsed = (now - since).num_seconds();
                elapsed >= *interval_secs as i64
            }
            ScheduleKind::Cron { pattern } => {
                // Only fire if we haven't fired this minute yet.
                let last_minute = self.last_fired.map(|t| t.format("%Y%m%d%H%M").to_string());
                let this_minute = now.format("%Y%m%d%H%M").to_string();
                if last_minute.as_deref() == Some(this_minute.as_str()) {
                    return false;
                }
                pattern.matches(now)
            }
        }
    }

    /// Record a fire event.
    pub fn record_fire(&mut self, now: DateTime<Utc>) {
        self.last_fired = Some(now);
        self.fire_count += 1;
        if let Some(max) = self.max_fires
            && self.fire_count >= max
        {
            self.status = ScheduleStatus::Expired;
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use serde_json::json;

    use super::*;

    #[test]
    fn once_fires_at_target_time() {
        let target = Utc::now() + Duration::seconds(10);
        let sched = Schedule::once("test", target, json!({"prompt": "hello"}));

        assert!(!sched.should_fire(Utc::now()));
        assert!(sched.should_fire(target));
        assert!(sched.should_fire(target + Duration::seconds(1)));
    }

    #[test]
    fn once_expires_after_fire() {
        let target = Utc::now();
        let mut sched = Schedule::once("test", target, json!({}));

        assert!(sched.should_fire(target));
        sched.record_fire(target);

        assert_eq!(sched.status, ScheduleStatus::Expired);
        assert!(!sched.should_fire(target + Duration::seconds(1)));
    }

    #[test]
    fn interval_fires_after_elapsed() {
        let mut sched = Schedule::interval("poll", 60, json!({}));
        // Pin created_at so the test isn't timing-sensitive.
        let base = Utc::now() - Duration::seconds(100);
        sched.created_at = base;

        // Shouldn't fire before the interval
        assert!(!sched.should_fire(base + Duration::seconds(30)));

        // Should fire after 60 seconds
        assert!(sched.should_fire(base + Duration::seconds(60)));
    }

    #[test]
    fn interval_repeats_after_fire() {
        let mut sched = Schedule::interval("poll", 30, json!({}));
        let base = Utc::now() - Duration::seconds(100);
        sched.created_at = base;

        let fire_time = base + Duration::seconds(30);
        assert!(sched.should_fire(fire_time));
        sched.record_fire(fire_time);

        // Not yet 30s after fire
        assert!(!sched.should_fire(fire_time + Duration::seconds(10)));
        // 30s after fire
        assert!(sched.should_fire(fire_time + Duration::seconds(30)));
    }

    #[test]
    fn paused_schedule_does_not_fire() {
        let target = Utc::now();
        let mut sched = Schedule::once("test", target, json!({}));
        sched.status = ScheduleStatus::Paused;
        assert!(!sched.should_fire(target));
    }

    #[test]
    fn max_fires_caps_interval() {
        let now = Utc::now();
        let mut sched = Schedule::interval("limited", 10, json!({}));
        sched.max_fires = Some(2);

        sched.record_fire(now + Duration::seconds(10));
        assert_eq!(sched.fire_count, 1);
        assert_eq!(sched.status, ScheduleStatus::Active);

        sched.record_fire(now + Duration::seconds(20));
        assert_eq!(sched.fire_count, 2);
        assert_eq!(sched.status, ScheduleStatus::Expired);
        assert!(!sched.should_fire(now + Duration::seconds(30)));
    }

    #[test]
    fn schedule_serializes_roundtrip() {
        let sched = Schedule::interval("test", 300, json!({"prompt": "run tests"}));
        let json = serde_json::to_string(&sched).unwrap();
        let parsed: Schedule = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "test");
        assert_eq!(parsed.fire_count, 0);
    }
}
