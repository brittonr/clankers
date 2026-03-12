//! Schedule engine — tick loop and event dispatch.
//!
//! The engine owns all active schedules and runs a background tick loop.
//! When a schedule fires, a `ScheduleEvent` is sent on a broadcast channel.
//! The consumer (daemon, interactive mode, tool) decides what to do.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use parking_lot::Mutex;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::info;

use crate::schedule::Schedule;
use crate::schedule::ScheduleId;
use crate::schedule::ScheduleStatus;

/// Event emitted when a schedule fires.
#[derive(Debug, Clone)]
pub struct ScheduleEvent {
    pub schedule_id: ScheduleId,
    pub schedule_name: String,
    pub payload: serde_json::Value,
    pub fire_count: u64,
}

/// Shared schedule state, protected by a mutex for cross-task access.
type Schedules = Arc<Mutex<HashMap<ScheduleId, Schedule>>>;

/// The schedule engine. Create one, add schedules, start the tick loop.
pub struct ScheduleEngine {
    schedules: Schedules,
    event_tx: broadcast::Sender<ScheduleEvent>,
    cancel: CancellationToken,
    /// Tick interval for the background loop. Default: 15 seconds.
    /// Cron granularity is 1 minute, so checking every 15s gives
    /// sub-minute latency without burning CPU.
    tick_interval: Duration,
}

impl ScheduleEngine {
    /// Create a new engine with a broadcast channel for events.
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            schedules: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
            cancel: CancellationToken::new(),
            tick_interval: Duration::from_secs(15),
        }
    }

    /// Override the tick interval (for tests or high-frequency schedules).
    pub fn with_tick_interval(mut self, interval: Duration) -> Self {
        self.tick_interval = interval;
        self
    }

    /// Subscribe to schedule fire events.
    pub fn subscribe(&self) -> broadcast::Receiver<ScheduleEvent> {
        self.event_tx.subscribe()
    }

    /// Add a schedule. Returns the schedule ID.
    pub fn add(&self, schedule: Schedule) -> ScheduleId {
        let id = schedule.id.clone();
        info!("schedule added: {} ({})", schedule.name, id);
        self.schedules.lock().insert(id.clone(), schedule);
        id
    }

    /// Remove a schedule by ID. Returns the removed schedule if found.
    pub fn remove(&self, id: &ScheduleId) -> Option<Schedule> {
        let removed = self.schedules.lock().remove(id);
        if let Some(s) = &removed {
            info!("schedule removed: {} ({})", s.name, id);
        }
        removed
    }

    /// Pause a schedule. Returns false if not found.
    pub fn pause(&self, id: &ScheduleId) -> bool {
        let mut scheds = self.schedules.lock();
        if let Some(s) = scheds.get_mut(id) {
            s.status = ScheduleStatus::Paused;
            info!("schedule paused: {} ({})", s.name, id);
            true
        } else {
            false
        }
    }

    /// Resume a paused schedule. Returns false if not found or not paused.
    pub fn resume(&self, id: &ScheduleId) -> bool {
        let mut scheds = self.schedules.lock();
        if let Some(s) = scheds.get_mut(id)
            && s.status == ScheduleStatus::Paused
        {
            s.status = ScheduleStatus::Active;
            info!("schedule resumed: {} ({})", s.name, id);
            return true;
        }
        false
    }

    /// Get a snapshot of all schedules.
    pub fn list(&self) -> Vec<Schedule> {
        self.schedules.lock().values().cloned().collect()
    }

    /// Get a single schedule by ID.
    pub fn get(&self, id: &ScheduleId) -> Option<Schedule> {
        self.schedules.lock().get(id).cloned()
    }

    /// Get the cancellation token for stopping the tick loop.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// Start the background tick loop. Returns a join handle.
    ///
    /// Call `cancel_token().cancel()` to stop the loop.
    pub fn start(&self) -> tokio::task::JoinHandle<()> {
        let schedules = Arc::clone(&self.schedules);
        let event_tx = self.event_tx.clone();
        let cancel = self.cancel.clone();
        let interval = self.tick_interval;

        tokio::spawn(async move {
            info!("scheduler tick loop started (interval: {:?})", interval);
            loop {
                tokio::select! {
                    () = tokio::time::sleep(interval) => {}
                    () = cancel.cancelled() => {
                        info!("scheduler tick loop stopped");
                        break;
                    }
                }

                let now = Utc::now();
                let mut to_fire = Vec::new();

                // Collect schedules that should fire.
                {
                    let scheds = schedules.lock();
                    for sched in scheds.values() {
                        if sched.should_fire(now) {
                            to_fire.push(sched.id.clone());
                        }
                    }
                }

                // Fire them (separate lock acquisition to avoid holding
                // across the channel send).
                for id in to_fire {
                    let event = {
                        let mut scheds = schedules.lock();
                        let Some(sched) = scheds.get_mut(&id) else {
                            continue;
                        };
                        sched.record_fire(now);
                        debug!("schedule fired: {} ({}) [count={}]", sched.name, id, sched.fire_count);
                        ScheduleEvent {
                            schedule_id: id.clone(),
                            schedule_name: sched.name.clone(),
                            payload: sched.payload.clone(),
                            fire_count: sched.fire_count,
                        }
                    };
                    if event_tx.send(event).is_err() {
                        // No receivers — that's fine, the schedule still records the fire.
                        debug!("schedule event dropped (no receivers)");
                    }
                }

                // GC expired schedules.
                {
                    let mut scheds = schedules.lock();
                    let before = scheds.len();
                    scheds.retain(|_, s| s.status != ScheduleStatus::Expired);
                    let removed = before - scheds.len();
                    if removed > 0 {
                        debug!("gc'd {} expired schedule(s)", removed);
                    }
                }
            }
        })
    }

    /// Run one tick manually (for testing without the background loop).
    pub fn tick(&self) {
        let now = Utc::now();
        let mut to_fire = Vec::new();

        {
            let scheds = self.schedules.lock();
            for sched in scheds.values() {
                if sched.should_fire(now) {
                    to_fire.push(sched.id.clone());
                }
            }
        }

        for id in to_fire {
            let event = {
                let mut scheds = self.schedules.lock();
                let Some(sched) = scheds.get_mut(&id) else {
                    continue;
                };
                sched.record_fire(now);
                ScheduleEvent {
                    schedule_id: id.clone(),
                    schedule_name: sched.name.clone(),
                    payload: sched.payload.clone(),
                    fire_count: sched.fire_count,
                }
            };
            let _ = self.event_tx.send(event);
        }

        // GC
        self.schedules.lock().retain(|_, s| s.status != ScheduleStatus::Expired);
    }
}

impl Default for ScheduleEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use serde_json::json;

    use super::*;

    #[test]
    fn add_and_list() {
        let engine = ScheduleEngine::new();
        let sched = Schedule::interval("test", 60, json!({"prompt": "check status"}));
        let id = engine.add(sched);

        let list = engine.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
    }

    #[test]
    fn remove_schedule() {
        let engine = ScheduleEngine::new();
        let sched = Schedule::interval("test", 60, json!({}));
        let id = engine.add(sched);
        assert_eq!(engine.list().len(), 1);

        let removed = engine.remove(&id);
        assert!(removed.is_some());
        assert_eq!(engine.list().len(), 0);
    }

    #[test]
    fn pause_and_resume() {
        let engine = ScheduleEngine::new();
        let sched = Schedule::interval("test", 60, json!({}));
        let id = engine.add(sched);

        assert!(engine.pause(&id));
        assert_eq!(engine.get(&id).unwrap().status, ScheduleStatus::Paused);

        assert!(engine.resume(&id));
        assert_eq!(engine.get(&id).unwrap().status, ScheduleStatus::Active);
    }

    #[test]
    fn tick_fires_due_schedule() {
        let engine = ScheduleEngine::new();
        let mut rx = engine.subscribe();

        // Create an interval schedule that should fire immediately
        // (interval=0 means "fire every tick since last_fired is None").
        let mut sched = Schedule::interval("immediate", 0, json!({"msg": "fired!"}));
        // Force last_fired to be far in the past so it fires on first tick.
        sched.last_fired = Some(Utc::now() - Duration::seconds(100));
        engine.add(sched);

        engine.tick();

        let event = rx.try_recv().expect("should receive fire event");
        assert_eq!(event.schedule_name, "immediate");
        assert_eq!(event.payload, json!({"msg": "fired!"}));
        assert_eq!(event.fire_count, 1);
    }

    #[test]
    fn tick_gc_expired() {
        let engine = ScheduleEngine::new();
        let target = Utc::now() - Duration::seconds(1);
        let sched = Schedule::once("one-shot", target, json!({}));
        engine.add(sched);

        assert_eq!(engine.list().len(), 1);
        engine.tick(); // fires + expires
        assert_eq!(engine.list().len(), 0); // gc'd
    }

    #[tokio::test]
    async fn background_loop_fires_events() {
        let engine = ScheduleEngine::new().with_tick_interval(std::time::Duration::from_millis(20));
        let mut rx = engine.subscribe();

        let mut sched = Schedule::interval("bg-test", 0, json!({}));
        sched.last_fired = Some(Utc::now() - Duration::seconds(100));
        engine.add(sched);

        let handle = engine.start();

        // Wait for at least one event.
        let event = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("should receive within 2s")
            .expect("should receive event");

        assert_eq!(event.schedule_name, "bg-test");

        engine.cancel_token().cancel();
        let _ = handle.await;
    }
}
