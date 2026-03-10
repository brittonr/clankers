//! Cron-like scheduling engine for clankers.
//!
//! Schedules are stored in memory and ticked by a background task. When a
//! schedule fires, the engine emits a `ScheduleEvent` on a broadcast channel.
//! The daemon or interactive mode decides what to do with the event (run a
//! prompt, execute a tool, etc.).
//!
//! Three schedule kinds:
//! - **Once** — fires at a specific datetime, then auto-removes itself.
//! - **Interval** — fires every N seconds/minutes/hours.
//! - **Cron** — fires on a cron-like pattern (minute, hour, day-of-week).
//!
//! Schedules can be paused, resumed, and deleted. Each carries an arbitrary
//! JSON payload that the consumer interprets (prompt text, tool params, etc.).

pub mod cron;
pub mod engine;
pub mod schedule;

pub use engine::ScheduleEngine;
pub use engine::ScheduleEvent;
pub use schedule::Schedule;
pub use schedule::ScheduleId;
pub use schedule::ScheduleKind;
pub use schedule::ScheduleStatus;
