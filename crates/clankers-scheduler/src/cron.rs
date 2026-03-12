//! Minimal cron-like pattern matching.
//!
//! Supports a subset of cron syntax:
//! - `*` — match any value
//! - `N` — match exact value
//! - `N,M` — match multiple values
//! - `N-M` — match a range (inclusive)
//! - `*/N` — match every Nth value
//!
//! Fields: minute (0-59), hour (0-23), day-of-week (0-6, 0=Sunday).
//! No month or day-of-month (schedules are week-periodic at most).

use serde::Deserialize;
use serde::Serialize;

/// A single cron field matcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CronField {
    /// Match any value.
    Any,
    /// Match a specific value.
    Exact(u32),
    /// Match any of these values.
    List(Vec<u32>),
    /// Match values in [start, end] inclusive.
    Range(u32, u32),
    /// Match every Nth value (starting from 0).
    Step(u32),
}

impl CronField {
    /// Parse a cron field string.
    pub fn parse(s: &str) -> Result<Self, CronParseError> {
        let s = s.trim();
        if s == "*" {
            return Ok(Self::Any);
        }
        if let Some(step) = s.strip_prefix("*/") {
            let n: u32 = step.parse().map_err(|_| CronParseError(format!("invalid step: {s}")))?;
            if n == 0 {
                return Err(CronParseError("step cannot be 0".into()));
            }
            return Ok(Self::Step(n));
        }
        if s.contains(',') {
            let values: Result<Vec<u32>, _> = s.split(',').map(|v| v.trim().parse::<u32>()).collect();
            return Ok(Self::List(values.map_err(|_| CronParseError(format!("invalid list: {s}")))?));
        }
        if s.contains('-') {
            let parts: Vec<&str> = s.splitn(2, '-').collect();
            let start: u32 =
                parts[0].trim().parse().map_err(|_| CronParseError(format!("invalid range start: {s}")))?;
            let end: u32 = parts[1].trim().parse().map_err(|_| CronParseError(format!("invalid range end: {s}")))?;
            return Ok(Self::Range(start, end));
        }
        let n: u32 = s.parse().map_err(|_| CronParseError(format!("invalid number: {s}")))?;
        Ok(Self::Exact(n))
    }

    /// Check if `value` matches this field.
    pub fn matches(&self, value: u32) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(n) => value == *n,
            Self::List(values) => values.contains(&value),
            Self::Range(start, end) => value >= *start && value <= *end,
            Self::Step(n) => value.is_multiple_of(*n),
        }
    }
}

/// Three-field cron pattern: minute, hour, day-of-week.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronPattern {
    pub minute: CronField,
    pub hour: CronField,
    pub day_of_week: CronField,
}

impl CronPattern {
    /// Parse a three-field cron string: "minute hour day_of_week".
    ///
    /// Examples:
    /// - `"0 * *"` — every hour on the hour
    /// - `"*/15 * *"` — every 15 minutes
    /// - `"0 9 1-5"` — 9am on weekdays (Mon-Fri)
    /// - `"30 14 *"` — 2:30pm daily
    pub fn parse(s: &str) -> Result<Self, CronParseError> {
        let fields: Vec<&str> = s.split_whitespace().collect();
        if fields.len() != 3 {
            return Err(CronParseError(format!("expected 3 fields (minute hour day_of_week), got {}", fields.len())));
        }
        Ok(Self {
            minute: CronField::parse(fields[0])?,
            hour: CronField::parse(fields[1])?,
            day_of_week: CronField::parse(fields[2])?,
        })
    }

    /// Check if a datetime matches this pattern.
    pub fn matches(&self, dt: chrono::DateTime<chrono::Utc>) -> bool {
        use chrono::Datelike;
        use chrono::Timelike;

        let minute = dt.minute();
        let hour = dt.hour();
        // chrono: Mon=0 .. Sun=6 from weekday().num_days_from_monday()
        // cron convention: Sun=0, Mon=1 .. Sat=6
        let dow = dt.weekday().num_days_from_sunday();

        self.minute.matches(minute) && self.hour.matches(hour) && self.day_of_week.matches(dow)
    }
}

/// Error from parsing a cron expression.
#[derive(Debug, Clone)]
pub struct CronParseError(pub String);

impl std::fmt::Display for CronParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cron parse error: {}", self.0)
    }
}

impl std::error::Error for CronParseError {}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn field_any_matches_everything() {
        let f = CronField::Any;
        assert!(f.matches(0));
        assert!(f.matches(59));
    }

    #[test]
    fn field_exact_matches_only_value() {
        let f = CronField::Exact(15);
        assert!(f.matches(15));
        assert!(!f.matches(14));
        assert!(!f.matches(16));
    }

    #[test]
    fn field_list_matches_members() {
        let f = CronField::List(vec![0, 15, 30, 45]);
        assert!(f.matches(0));
        assert!(f.matches(30));
        assert!(!f.matches(10));
    }

    #[test]
    fn field_range_matches_inclusive() {
        let f = CronField::Range(1, 5);
        assert!(!f.matches(0));
        assert!(f.matches(1));
        assert!(f.matches(3));
        assert!(f.matches(5));
        assert!(!f.matches(6));
    }

    #[test]
    fn field_step_matches_multiples() {
        let f = CronField::Step(15);
        assert!(f.matches(0));
        assert!(f.matches(15));
        assert!(f.matches(30));
        assert!(f.matches(45));
        assert!(!f.matches(7));
    }

    #[test]
    fn parse_field_star() {
        assert_eq!(CronField::parse("*").unwrap(), CronField::Any);
    }

    #[test]
    fn parse_field_exact() {
        assert_eq!(CronField::parse("42").unwrap(), CronField::Exact(42));
    }

    #[test]
    fn parse_field_step() {
        assert_eq!(CronField::parse("*/10").unwrap(), CronField::Step(10));
    }

    #[test]
    fn parse_field_step_zero_error() {
        assert!(CronField::parse("*/0").is_err());
    }

    #[test]
    fn parse_field_list() {
        assert_eq!(CronField::parse("1,3,5").unwrap(), CronField::List(vec![1, 3, 5]));
    }

    #[test]
    fn parse_field_range() {
        assert_eq!(CronField::parse("1-5").unwrap(), CronField::Range(1, 5));
    }

    #[test]
    fn parse_pattern_every_hour() {
        let p = CronPattern::parse("0 * *").unwrap();
        assert_eq!(p.minute, CronField::Exact(0));
        assert_eq!(p.hour, CronField::Any);
        assert_eq!(p.day_of_week, CronField::Any);
    }

    #[test]
    fn parse_pattern_weekday_morning() {
        let p = CronPattern::parse("0 9 1-5").unwrap();
        // 2026-03-10 is a Tuesday (dow=2)
        let tuesday_9am = chrono::Utc.with_ymd_and_hms(2026, 3, 10, 9, 0, 0).unwrap();
        assert!(p.matches(tuesday_9am));

        let tuesday_10am = chrono::Utc.with_ymd_and_hms(2026, 3, 10, 10, 0, 0).unwrap();
        assert!(!p.matches(tuesday_10am));

        // Sunday (dow=0) should not match 1-5
        let sunday_9am = chrono::Utc.with_ymd_and_hms(2026, 3, 8, 9, 0, 0).unwrap();
        assert!(!p.matches(sunday_9am));
    }

    #[test]
    fn parse_pattern_every_15_min() {
        let p = CronPattern::parse("*/15 * *").unwrap();
        let at_00 = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        let at_15 = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 12, 15, 0).unwrap();
        let at_07 = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 12, 7, 0).unwrap();

        assert!(p.matches(at_00));
        assert!(p.matches(at_15));
        assert!(!p.matches(at_07));
    }

    #[test]
    fn parse_pattern_wrong_field_count() {
        assert!(CronPattern::parse("0 *").is_err());
        assert!(CronPattern::parse("0 * * *").is_err());
    }
}
