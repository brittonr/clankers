//! iCalendar (RFC 5545) parser and generator.
//!
//! Parses VCALENDAR text into `Event` structs and generates valid
//! VCALENDAR text from Events. No external date/time crates — this
//! runs in WASM with minimal dependencies.

use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════
//  Data types
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub uid: String,
    pub summary: String,
    pub start: CalDateTime,
    pub end: Option<CalDateTime>,
    pub duration: Option<String>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub attendees: Vec<String>,
    pub calendar: String,
    pub etag: Option<String>,
    pub href: Option<String>,
    pub all_day: bool,
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalDateTime {
    pub timestamp: String,
    pub timezone: Option<String>,
    pub date_only: bool,
}

// ═══════════════════════════════════════════════════════════════════════
//  Line unfolding (RFC 5545 §3.1)
// ═══════════════════════════════════════════════════════════════════════

/// Unfold content lines per RFC 5545 §3.1.
/// Lines beginning with a space or tab are continuations of the previous line.
pub fn unfold_lines(input: &str) -> String {
    // Normalize line endings to LF
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut result = String::with_capacity(normalized.len());
    let mut first = true;

    for line in normalized.split('\n') {
        if first {
            result.push_str(line);
            first = false;
        } else if line.starts_with(' ') || line.starts_with('\t') {
            // Continuation: strip the leading whitespace and append
            result.push_str(&line[1..]);
        } else {
            result.push('\n');
            result.push_str(line);
        }
    }
    result
}

// ═══════════════════════════════════════════════════════════════════════
//  Text escaping/unescaping
// ═══════════════════════════════════════════════════════════════════════

/// Unescape iCalendar text values.
pub fn unescape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('\\') => {
                    result.push('\\');
                    chars.next();
                }
                Some('n') | Some('N') => {
                    result.push('\n');
                    chars.next();
                }
                Some(',') => {
                    result.push(',');
                    chars.next();
                }
                Some(';') => {
                    result.push(';');
                    chars.next();
                }
                _ => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Escape text for iCalendar output.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace(',', "\\,")
        .replace(';', "\\;")
}

// ═══════════════════════════════════════════════════════════════════════
//  Parsing
// ═══════════════════════════════════════════════════════════════════════

/// Parse VCALENDAR text, extract all VEVENT components into Events.
pub fn parse_events(ical_text: &str) -> Vec<Event> {
    let unfolded = unfold_lines(ical_text);
    let mut events = Vec::new();
    let mut in_vevent = false;
    let mut uid = String::new();
    let mut summary = String::new();
    let mut dtstart: Option<CalDateTime> = None;
    let mut dtend: Option<CalDateTime> = None;
    let mut duration: Option<String> = None;
    let mut location: Option<String> = None;
    let mut description: Option<String> = None;
    let mut attendees: Vec<String> = Vec::new();
    let mut status: Option<String> = None;

    for line in unfolded.lines() {
        let trimmed = line.trim();

        if trimmed == "BEGIN:VEVENT" {
            in_vevent = true;
            uid.clear();
            summary.clear();
            dtstart = None;
            dtend = None;
            duration = None;
            location = None;
            description = None;
            attendees.clear();
            status = None;
            continue;
        }

        if trimmed == "END:VEVENT" {
            if in_vevent {
                let all_day = dtstart.as_ref().is_some_and(|dt| dt.date_only);

                // Compute end from duration if DTEND missing
                let computed_end = if dtend.is_none() {
                    if let (Some(ref start), Some(ref dur)) = (&dtstart, &duration) {
                        if let Some(mins) = duration_to_minutes(dur) {
                            Some(add_minutes_to_datetime(start, mins))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    dtend.clone()
                };

                events.push(Event {
                    uid: uid.clone(),
                    summary: summary.clone(),
                    start: dtstart.clone().unwrap_or(CalDateTime {
                        timestamp: String::new(),
                        timezone: None,
                        date_only: false,
                    }),
                    end: computed_end,
                    duration: duration.clone(),
                    location: location.clone(),
                    description: description.clone(),
                    attendees: attendees.clone(),
                    calendar: String::new(),
                    etag: None,
                    href: None,
                    all_day,
                    status: status.clone(),
                });
            }
            in_vevent = false;
            continue;
        }

        if !in_vevent {
            continue;
        }

        // Parse property
        if let Some(val) = strip_prop(trimmed, "UID") {
            uid = val.to_string();
        } else if let Some(val) = strip_prop(trimmed, "SUMMARY") {
            summary = unescape(val);
        } else if trimmed.starts_with("DTSTART") {
            dtstart = Some(parse_datetime_prop(trimmed));
        } else if trimmed.starts_with("DTEND") {
            dtend = Some(parse_datetime_prop(trimmed));
        } else if let Some(val) = strip_prop(trimmed, "DURATION") {
            duration = Some(val.to_string());
        } else if let Some(val) = strip_prop(trimmed, "LOCATION") {
            location = Some(unescape(val));
        } else if let Some(val) = strip_prop(trimmed, "DESCRIPTION") {
            description = Some(unescape(val));
        } else if trimmed.starts_with("ATTENDEE") {
            if let Some(email) = extract_mailto(trimmed) {
                attendees.push(email);
            }
        } else if let Some(val) = strip_prop(trimmed, "STATUS") {
            status = Some(val.to_string());
        }
    }

    events
}

/// Strip a simple property name and return its value.
/// Handles "PROP:value" format.
fn strip_prop<'a>(line: &'a str, prop: &str) -> Option<&'a str> {
    if line.starts_with(prop) {
        let rest = &line[prop.len()..];
        if rest.starts_with(':') {
            return Some(&rest[1..]);
        }
    }
    None
}

/// Parse a DTSTART or DTEND property line into CalDateTime.
fn parse_datetime_prop(line: &str) -> CalDateTime {
    // Find the colon separating params from value
    let colon_pos = match line.find(':') {
        Some(p) => p,
        None => {
            return CalDateTime {
                timestamp: String::new(),
                timezone: None,
                date_only: false,
            };
        }
    };

    let params = &line[..colon_pos];
    let value = &line[colon_pos + 1..];

    // Check for VALUE=DATE
    let date_only = params.contains("VALUE=DATE");

    // Check for TZID=
    let timezone = if let Some(tz_start) = params.find("TZID=") {
        let tz_rest = &params[tz_start + 5..];
        // TZID ends at ; or end of params
        let tz_end = tz_rest.find(';').unwrap_or(tz_rest.len());
        Some(tz_rest[..tz_end].to_string())
    } else if value.ends_with('Z') {
        Some("UTC".to_string())
    } else {
        None
    };

    // Strip trailing Z from value
    let timestamp = value.trim_end_matches('Z').to_string();

    CalDateTime {
        timestamp,
        timezone,
        date_only,
    }
}

/// Extract email from ATTENDEE property (mailto: URI).
fn extract_mailto(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    if let Some(pos) = lower.find("mailto:") {
        let email_start = pos + 7;
        let rest = &line[email_start..];
        // Email ends at whitespace, ;, or end of line
        let end = rest
            .find(|c: char| c.is_whitespace() || c == ';')
            .unwrap_or(rest.len());
        let email = rest[..end].trim().to_string();
        if !email.is_empty() {
            return Some(email);
        }
    }
    None
}

// ═══════════════════════════════════════════════════════════════════════
//  Generation
// ═══════════════════════════════════════════════════════════════════════

/// Generate valid RFC 5545 VCALENDAR text from an Event.
/// `dtstamp` is passed in for deterministic output (e.g., "20260303T120000Z").
pub fn generate_vcalendar(event: &Event, dtstamp: &str) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines.push("BEGIN:VCALENDAR".to_string());
    lines.push("VERSION:2.0".to_string());
    lines.push("PRODID:-//clankers//calendar-plugin//EN".to_string());
    lines.push("BEGIN:VEVENT".to_string());
    lines.push(format!("UID:{}", event.uid));
    lines.push(format!("DTSTAMP:{dtstamp}"));

    // DTSTART
    if event.all_day {
        lines.push(format!("DTSTART;VALUE=DATE:{}", event.start.timestamp));
    } else if let Some(ref tz) = event.start.timezone {
        if tz == "UTC" {
            lines.push(format!("DTSTART:{}Z", event.start.timestamp));
        } else {
            lines.push(format!("DTSTART;TZID={}:{}", tz, event.start.timestamp));
        }
    } else {
        lines.push(format!("DTSTART:{}", event.start.timestamp));
    }

    // DTEND
    if let Some(ref end) = event.end {
        if event.all_day {
            lines.push(format!("DTEND;VALUE=DATE:{}", end.timestamp));
        } else if let Some(ref tz) = end.timezone {
            if tz == "UTC" {
                lines.push(format!("DTEND:{}Z", end.timestamp));
            } else {
                lines.push(format!("DTEND;TZID={}:{}", tz, end.timestamp));
            }
        } else {
            lines.push(format!("DTEND:{}", end.timestamp));
        }
    } else if let Some(ref dur) = event.duration {
        lines.push(format!("DURATION:{dur}"));
    }

    lines.push(format!("SUMMARY:{}", escape(&event.summary)));

    if let Some(ref loc) = event.location {
        lines.push(format!("LOCATION:{}", escape(loc)));
    }
    if let Some(ref desc) = event.description {
        lines.push(format!("DESCRIPTION:{}", escape(desc)));
    }
    if let Some(ref st) = event.status {
        lines.push(format!("STATUS:{st}"));
    }
    for attendee in &event.attendees {
        lines.push(format!("ATTENDEE:mailto:{attendee}"));
    }

    lines.push("END:VEVENT".to_string());
    lines.push("END:VCALENDAR".to_string());

    // Fold lines and join with CRLF
    lines
        .into_iter()
        .map(|l| fold_line(&l))
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// Fold a line to max 75 octets per RFC 5545.
fn fold_line(line: &str) -> String {
    let bytes = line.as_bytes();
    if bytes.len() <= 75 {
        return line.to_string();
    }

    let mut result = String::new();
    let mut pos = 0;
    let mut first = true;

    while pos < bytes.len() {
        let max_len = if first { 75 } else { 74 }; // continuation lines lose 1 byte to the space
        let end = (pos + max_len).min(bytes.len());

        // Don't split in the middle of a multi-byte UTF-8 character
        let mut safe_end = end;
        while safe_end > pos && !is_char_boundary(bytes, safe_end) {
            safe_end -= 1;
        }
        if safe_end == pos {
            safe_end = end; // fallback
        }

        if !first {
            result.push_str("\r\n ");
        }
        result.push_str(&String::from_utf8_lossy(&bytes[pos..safe_end]));
        pos = safe_end;
        first = false;
    }

    result
}

fn is_char_boundary(bytes: &[u8], index: usize) -> bool {
    if index >= bytes.len() {
        return true;
    }
    // A byte is a char boundary if it's not a continuation byte (10xxxxxx)
    bytes[index] & 0xC0 != 0x80
}

// ═══════════════════════════════════════════════════════════════════════
//  Duration handling
// ═══════════════════════════════════════════════════════════════════════

/// Convert human-friendly duration to ISO 8601 duration.
/// "2h" → "PT2H", "30m" → "PT30M", "1h30m" → "PT1H30M", "1d" → "P1D"
pub fn parse_human_duration(s: &str) -> Option<String> {
    let s = s.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }

    let mut days: u64 = 0;
    let mut hours: u64 = 0;
    let mut minutes: u64 = 0;
    let mut num_buf = String::new();
    let mut found_any = false;

    for c in s.chars() {
        if c.is_ascii_digit() {
            num_buf.push(c);
        } else {
            let n: u64 = num_buf.parse().ok()?;
            num_buf.clear();
            match c {
                'd' => {
                    days = n;
                    found_any = true;
                }
                'h' => {
                    hours = n;
                    found_any = true;
                }
                'm' => {
                    minutes = n;
                    found_any = true;
                }
                _ => return None,
            }
        }
    }

    if !found_any {
        return None;
    }

    let mut result = String::from("P");
    if days > 0 {
        result.push_str(&format!("{days}D"));
    }
    if hours > 0 || minutes > 0 {
        result.push('T');
        if hours > 0 {
            result.push_str(&format!("{hours}H"));
        }
        if minutes > 0 {
            result.push_str(&format!("{minutes}M"));
        }
    }
    // Edge case: "P" alone if everything is 0 — shouldn't happen but handle
    if result == "P" {
        return Some("PT0M".to_string());
    }

    Some(result)
}

/// Parse ISO 8601 duration to total minutes.
/// PT1H → 60, PT30M → 30, PT1H30M → 90, P1D → 1440
pub fn duration_to_minutes(iso_dur: &str) -> Option<u64> {
    let s = iso_dur.trim();
    if !s.starts_with('P') {
        return None;
    }

    let s = &s[1..]; // strip P
    let mut total_minutes: u64 = 0;
    let mut num_buf = String::new();
    let mut in_time = false;

    for c in s.chars() {
        if c == 'T' {
            in_time = true;
            continue;
        }
        if c.is_ascii_digit() {
            num_buf.push(c);
        } else {
            let n: u64 = num_buf.parse().ok()?;
            num_buf.clear();
            match (c, in_time) {
                ('D', false) => total_minutes += n * 1440,
                ('W', false) => total_minutes += n * 10080,
                ('H', true) => total_minutes += n * 60,
                ('M', true) => total_minutes += n,
                ('S', true) => total_minutes += n / 60, // round down
                _ => return None,
            }
        }
    }

    Some(total_minutes)
}

/// Add minutes to a CalDateTime. Simple arithmetic on YYYYMMDDTHHMMSS.
pub fn add_minutes_to_datetime(dt: &CalDateTime, minutes: u64) -> CalDateTime {
    let ts = &dt.timestamp;

    if dt.date_only || ts.len() < 8 {
        // For date-only, add as days
        let days = minutes / 1440;
        if days > 0 {
            let new_ts = add_days_to_date(ts, days);
            return CalDateTime {
                timestamp: new_ts,
                timezone: dt.timezone.clone(),
                date_only: dt.date_only,
            };
        }
        return dt.clone();
    }

    // Parse YYYYMMDDTHHMMSS
    let (date_part, time_part) = if let Some(t_pos) = ts.find('T') {
        (&ts[..t_pos], &ts[t_pos + 1..])
    } else {
        return dt.clone();
    };

    let year: u32 = date_part.get(0..4).and_then(|s| s.parse().ok()).unwrap_or(2026);
    let month: u32 = date_part.get(4..6).and_then(|s| s.parse().ok()).unwrap_or(1);
    let day: u32 = date_part.get(6..8).and_then(|s| s.parse().ok()).unwrap_or(1);
    let hour: u32 = time_part.get(0..2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let minute: u32 = time_part.get(2..4).and_then(|s| s.parse().ok()).unwrap_or(0);
    let second: u32 = time_part.get(4..6).and_then(|s| s.parse().ok()).unwrap_or(0);

    let total_mins = hour * 60 + minute + minutes as u32;
    let new_hour = (total_mins / 60) % 24;
    let new_minute = total_mins % 60;
    let extra_days = total_mins / 1440;

    let (new_year, new_month, new_day) = if extra_days > 0 {
        add_days(year, month, day, extra_days)
    } else {
        (year, month, day)
    };

    let new_ts = format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}",
        new_year, new_month, new_day, new_hour, new_minute, second
    );

    CalDateTime {
        timestamp: new_ts,
        timezone: dt.timezone.clone(),
        date_only: false,
    }
}

fn add_days_to_date(date_str: &str, days: u64) -> String {
    let year: u32 = date_str.get(0..4).and_then(|s| s.parse().ok()).unwrap_or(2026);
    let month: u32 = date_str.get(4..6).and_then(|s| s.parse().ok()).unwrap_or(1);
    let day: u32 = date_str.get(6..8).and_then(|s| s.parse().ok()).unwrap_or(1);

    let (y, m, d) = add_days(year, month, day, days as u32);
    format!("{:04}{:02}{:02}", y, m, d)
}

fn add_days(mut year: u32, mut month: u32, mut day: u32, mut extra_days: u32) -> (u32, u32, u32) {
    while extra_days > 0 {
        let days_in_month = days_in_month(year, month);
        let remaining = days_in_month - day;
        if extra_days <= remaining {
            day += extra_days;
            extra_days = 0;
        } else {
            extra_days -= remaining + 1;
            day = 1;
            month += 1;
            if month > 12 {
                month = 1;
                year += 1;
            }
        }
    }
    (year, month, day)
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 => 31,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        3 => 31,
        4 => 30,
        5 => 31,
        6 => 30,
        7 => 31,
        8 => 31,
        9 => 30,
        10 => 31,
        11 => 30,
        12 => 31,
        _ => 30,
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Display formatting
// ═══════════════════════════════════════════════════════════════════════

/// Format a time range for display.
/// All day: "All day"
/// Both times: "10:00–10:30"
/// Start only: "10:00"
pub fn format_time_range(start: &CalDateTime, end: &Option<CalDateTime>, all_day: bool) -> String {
    if all_day {
        return "All day".to_string();
    }

    let start_hm = extract_hm(&start.timestamp);
    match end {
        Some(e) => {
            let end_hm = extract_hm(&e.timestamp);
            format!("{start_hm}–{end_hm}")
        }
        None => start_hm,
    }
}

/// Extract HH:MM from a timestamp like "20260303T100000" or "100000".
fn extract_hm(ts: &str) -> String {
    let time_part = if let Some(pos) = ts.find('T') {
        &ts[pos + 1..]
    } else if ts.len() >= 4 {
        ts
    } else {
        return ts.to_string();
    };

    let hour = time_part.get(0..2).unwrap_or("00");
    let min = time_part.get(2..4).unwrap_or("00");
    format!("{hour}:{min}")
}

// ═══════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── unfold_lines ────────────────────────────────────────────────

    #[test]
    fn unfold_crlf_continuation() {
        let input = "DESCRIPTION:This is a\r\n  long description\r\n  that spans lines";
        let result = unfold_lines(input);
        assert_eq!(result, "DESCRIPTION:This is a long description that spans lines");
    }

    #[test]
    fn unfold_tab_continuation() {
        let input = "SUMMARY:Hello\n\tWorld";
        let result = unfold_lines(input);
        assert_eq!(result, "SUMMARY:HelloWorld");
    }

    #[test]
    fn unfold_no_continuation() {
        let input = "SUMMARY:Hello\nDTSTART:20260303T100000Z";
        let result = unfold_lines(input);
        assert_eq!(result, "SUMMARY:Hello\nDTSTART:20260303T100000Z");
    }

    // ── unescape ────────────────────────────────────────────────────

    #[test]
    fn unescape_all_sequences() {
        assert_eq!(unescape(r"hello\\world"), "hello\\world");
        assert_eq!(unescape(r"line1\nline2"), "line1\nline2");
        assert_eq!(unescape(r"line1\Nline2"), "line1\nline2");
        assert_eq!(unescape(r"a\,b"), "a,b");
        assert_eq!(unescape(r"a\;b"), "a;b");
    }

    #[test]
    fn unescape_combined() {
        assert_eq!(unescape(r"a\,b\;c\\d\ne"), "a,b;c\\d\ne");
    }

    // ── parse_events ────────────────────────────────────────────────

    #[test]
    fn parse_simple_vevent() {
        let ical = "\
BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
BEGIN:VEVENT\r\n\
UID:test-123\r\n\
SUMMARY:Team Standup\r\n\
DTSTART:20260303T100000Z\r\n\
DTEND:20260303T103000Z\r\n\
LOCATION:Zoom\r\n\
END:VEVENT\r\n\
END:VCALENDAR";

        let events = parse_events(ical);
        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(e.uid, "test-123");
        assert_eq!(e.summary, "Team Standup");
        assert_eq!(e.start.timestamp, "20260303T100000");
        assert_eq!(e.start.timezone, Some("UTC".to_string()));
        assert_eq!(e.end.as_ref().unwrap().timestamp, "20260303T103000");
        assert_eq!(e.location, Some("Zoom".to_string()));
        assert!(!e.all_day);
    }

    #[test]
    fn parse_tzid_datetime() {
        let ical = "\
BEGIN:VCALENDAR\r\n\
BEGIN:VEVENT\r\n\
UID:tz-1\r\n\
SUMMARY:Meeting\r\n\
DTSTART;TZID=America/New_York:20260304T140000\r\n\
DTEND;TZID=America/New_York:20260304T160000\r\n\
END:VEVENT\r\n\
END:VCALENDAR";

        let events = parse_events(ical);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].start.timezone, Some("America/New_York".to_string()));
        assert_eq!(events[0].start.timestamp, "20260304T140000");
        assert!(!events[0].start.date_only);
    }

    #[test]
    fn parse_value_date_all_day() {
        let ical = "\
BEGIN:VCALENDAR\r\n\
BEGIN:VEVENT\r\n\
UID:allday-1\r\n\
SUMMARY:Holiday\r\n\
DTSTART;VALUE=DATE:20260304\r\n\
DTEND;VALUE=DATE:20260305\r\n\
END:VEVENT\r\n\
END:VCALENDAR";

        let events = parse_events(ical);
        assert_eq!(events.len(), 1);
        assert!(events[0].all_day);
        assert!(events[0].start.date_only);
        assert_eq!(events[0].start.timestamp, "20260304");
    }

    #[test]
    fn parse_floating_datetime() {
        let ical = "\
BEGIN:VCALENDAR\r\n\
BEGIN:VEVENT\r\n\
UID:float-1\r\n\
SUMMARY:Local\r\n\
DTSTART:20260304T140000\r\n\
END:VEVENT\r\n\
END:VCALENDAR";

        let events = parse_events(ical);
        assert_eq!(events[0].start.timezone, None);
        assert_eq!(events[0].start.timestamp, "20260304T140000");
    }

    #[test]
    fn parse_duration_instead_of_dtend() {
        let ical = "\
BEGIN:VCALENDAR\r\n\
BEGIN:VEVENT\r\n\
UID:dur-1\r\n\
SUMMARY:Talk\r\n\
DTSTART:20260303T140000Z\r\n\
DURATION:PT1H30M\r\n\
END:VEVENT\r\n\
END:VCALENDAR";

        let events = parse_events(ical);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].duration, Some("PT1H30M".to_string()));
        let end = events[0].end.as_ref().unwrap();
        assert_eq!(end.timestamp, "20260303T153000");
    }

    #[test]
    fn parse_multiple_vevents() {
        let ical = "\
BEGIN:VCALENDAR\r\n\
BEGIN:VEVENT\r\n\
UID:1\r\n\
SUMMARY:First\r\n\
DTSTART:20260303T100000Z\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:2\r\n\
SUMMARY:Second\r\n\
DTSTART:20260303T140000Z\r\n\
END:VEVENT\r\n\
END:VCALENDAR";

        let events = parse_events(ical);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].uid, "1");
        assert_eq!(events[1].uid, "2");
    }

    #[test]
    fn parse_attendees() {
        let ical = "\
BEGIN:VCALENDAR\r\n\
BEGIN:VEVENT\r\n\
UID:att-1\r\n\
SUMMARY:Sync\r\n\
DTSTART:20260303T100000Z\r\n\
ATTENDEE;CN=Alice:mailto:alice@example.com\r\n\
ATTENDEE;CN=Bob:mailto:bob@example.com\r\n\
END:VEVENT\r\n\
END:VCALENDAR";

        let events = parse_events(ical);
        assert_eq!(events[0].attendees.len(), 2);
        assert_eq!(events[0].attendees[0], "alice@example.com");
        assert_eq!(events[0].attendees[1], "bob@example.com");
    }

    #[test]
    fn parse_escaped_text() {
        let ical = "\
BEGIN:VCALENDAR\r\n\
BEGIN:VEVENT\r\n\
UID:esc-1\r\n\
SUMMARY:Meeting\\, Important\r\n\
DESCRIPTION:Line 1\\nLine 2\\nLine 3\r\n\
DTSTART:20260303T100000Z\r\n\
END:VEVENT\r\n\
END:VCALENDAR";

        let events = parse_events(ical);
        assert_eq!(events[0].summary, "Meeting, Important");
        assert_eq!(events[0].description, Some("Line 1\nLine 2\nLine 3".to_string()));
    }

    #[test]
    fn parse_minimal_event() {
        let ical = "\
BEGIN:VCALENDAR\r\n\
BEGIN:VEVENT\r\n\
UID:min-1\r\n\
SUMMARY:Minimal\r\n\
DTSTART:20260303T100000Z\r\n\
END:VEVENT\r\n\
END:VCALENDAR";

        let events = parse_events(ical);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].location, None);
        assert_eq!(events[0].description, None);
        assert!(events[0].attendees.is_empty());
        assert_eq!(events[0].status, None);
        assert_eq!(events[0].end, None);
    }

    #[test]
    fn parse_google_format() {
        // Google Calendar adds extra properties that should be ignored
        let ical = "\
BEGIN:VCALENDAR\r\n\
PRODID:-//Google Inc//Google Calendar 70.9054//EN\r\n\
VERSION:2.0\r\n\
CALSCALE:GREGORIAN\r\n\
BEGIN:VEVENT\r\n\
UID:abcd1234@google.com\r\n\
SUMMARY:Sprint Planning\r\n\
DTSTART:20260303T140000Z\r\n\
DTEND:20260303T150000Z\r\n\
CREATED:20260301T120000Z\r\n\
LAST-MODIFIED:20260302T080000Z\r\n\
SEQUENCE:0\r\n\
STATUS:CONFIRMED\r\n\
TRANSP:OPAQUE\r\n\
END:VEVENT\r\n\
END:VCALENDAR";

        let events = parse_events(ical);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].uid, "abcd1234@google.com");
        assert_eq!(events[0].summary, "Sprint Planning");
        assert_eq!(events[0].status, Some("CONFIRMED".to_string()));
    }

    // ── generate_vcalendar ──────────────────────────────────────────

    #[test]
    fn generate_and_parse_roundtrip() {
        let event = Event {
            uid: "rt-1".to_string(),
            summary: "Roundtrip Test".to_string(),
            start: CalDateTime {
                timestamp: "20260304T140000".to_string(),
                timezone: Some("America/New_York".to_string()),
                date_only: false,
            },
            end: Some(CalDateTime {
                timestamp: "20260304T160000".to_string(),
                timezone: Some("America/New_York".to_string()),
                date_only: false,
            }),
            duration: None,
            location: Some("Room 42".to_string()),
            description: Some("Important meeting".to_string()),
            attendees: vec!["alice@example.com".to_string()],
            calendar: String::new(),
            etag: None,
            href: None,
            all_day: false,
            status: Some("CONFIRMED".to_string()),
        };

        let ical = generate_vcalendar(&event, "20260303T120000Z");
        let parsed = parse_events(&ical);

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].uid, "rt-1");
        assert_eq!(parsed[0].summary, "Roundtrip Test");
        assert_eq!(parsed[0].location, Some("Room 42".to_string()));
        assert_eq!(parsed[0].attendees, vec!["alice@example.com"]);
    }

    #[test]
    fn generate_all_day_event() {
        let event = Event {
            uid: "ad-1".to_string(),
            summary: "Holiday".to_string(),
            start: CalDateTime {
                timestamp: "20260304".to_string(),
                timezone: None,
                date_only: true,
            },
            end: Some(CalDateTime {
                timestamp: "20260305".to_string(),
                timezone: None,
                date_only: true,
            }),
            duration: None,
            location: None,
            description: None,
            attendees: vec![],
            calendar: String::new(),
            etag: None,
            href: None,
            all_day: true,
            status: None,
        };

        let ical = generate_vcalendar(&event, "20260303T120000Z");
        assert!(ical.contains("DTSTART;VALUE=DATE:20260304"));
        assert!(ical.contains("DTEND;VALUE=DATE:20260305"));
    }

    #[test]
    fn generate_minimal_event() {
        let event = Event {
            uid: "min-g".to_string(),
            summary: "Minimal".to_string(),
            start: CalDateTime {
                timestamp: "20260303T100000".to_string(),
                timezone: None,
                date_only: false,
            },
            end: None,
            duration: None,
            location: None,
            description: None,
            attendees: vec![],
            calendar: String::new(),
            etag: None,
            href: None,
            all_day: false,
            status: None,
        };

        let ical = generate_vcalendar(&event, "20260303T120000Z");
        assert!(ical.contains("SUMMARY:Minimal"));
        assert!(!ical.contains("LOCATION:"));
        assert!(!ical.contains("DESCRIPTION:"));
        assert!(!ical.contains("ATTENDEE:"));
    }

    // ── fold_line ───────────────────────────────────────────────────

    #[test]
    fn fold_short_line() {
        assert_eq!(fold_line("SUMMARY:Short"), "SUMMARY:Short");
    }

    #[test]
    fn fold_long_line() {
        let long = format!("DESCRIPTION:{}", "x".repeat(100));
        let folded = fold_line(&long);
        // Should contain continuation lines
        assert!(folded.contains("\r\n "));
        // First segment should be ≤75 bytes
        let first_line = folded.split("\r\n").next().unwrap();
        assert!(first_line.len() <= 75);
    }

    // ── parse_human_duration ────────────────────────────────────────

    #[test]
    fn human_duration_hours() {
        assert_eq!(parse_human_duration("2h"), Some("PT2H".to_string()));
    }

    #[test]
    fn human_duration_minutes() {
        assert_eq!(parse_human_duration("30m"), Some("PT30M".to_string()));
    }

    #[test]
    fn human_duration_hours_minutes() {
        assert_eq!(parse_human_duration("1h30m"), Some("PT1H30M".to_string()));
    }

    #[test]
    fn human_duration_days() {
        assert_eq!(parse_human_duration("1d"), Some("P1D".to_string()));
    }

    #[test]
    fn human_duration_days_hours() {
        assert_eq!(parse_human_duration("1d2h"), Some("P1DT2H".to_string()));
    }

    #[test]
    fn human_duration_full() {
        assert_eq!(parse_human_duration("1d2h30m"), Some("P1DT2H30M".to_string()));
    }

    #[test]
    fn human_duration_invalid() {
        assert_eq!(parse_human_duration("abc"), None);
        assert_eq!(parse_human_duration(""), None);
    }

    // ── duration_to_minutes ─────────────────────────────────────────

    #[test]
    fn iso_duration_to_minutes() {
        assert_eq!(duration_to_minutes("PT1H"), Some(60));
        assert_eq!(duration_to_minutes("PT30M"), Some(30));
        assert_eq!(duration_to_minutes("PT1H30M"), Some(90));
        assert_eq!(duration_to_minutes("P1D"), Some(1440));
        assert_eq!(duration_to_minutes("P1DT2H"), Some(1560));
    }

    #[test]
    fn iso_duration_invalid() {
        assert_eq!(duration_to_minutes("not a duration"), None);
    }

    // ── add_minutes_to_datetime ─────────────────────────────────────

    #[test]
    fn add_minutes_same_day() {
        let dt = CalDateTime {
            timestamp: "20260303T140000".to_string(),
            timezone: Some("UTC".to_string()),
            date_only: false,
        };
        let result = add_minutes_to_datetime(&dt, 90);
        assert_eq!(result.timestamp, "20260303T153000");
        assert_eq!(result.timezone, Some("UTC".to_string()));
    }

    #[test]
    fn add_minutes_day_overflow() {
        let dt = CalDateTime {
            timestamp: "20260303T230000".to_string(),
            timezone: None,
            date_only: false,
        };
        let result = add_minutes_to_datetime(&dt, 120);
        assert_eq!(result.timestamp, "20260304T010000");
    }

    // ── format_time_range ───────────────────────────────────────────

    #[test]
    fn format_all_day() {
        let start = CalDateTime {
            timestamp: "20260303".to_string(),
            timezone: None,
            date_only: true,
        };
        assert_eq!(format_time_range(&start, &None, true), "All day");
    }

    #[test]
    fn format_start_end() {
        let start = CalDateTime {
            timestamp: "20260303T100000".to_string(),
            timezone: None,
            date_only: false,
        };
        let end = CalDateTime {
            timestamp: "20260303T103000".to_string(),
            timezone: None,
            date_only: false,
        };
        assert_eq!(format_time_range(&start, &Some(end), false), "10:00–10:30");
    }

    #[test]
    fn format_start_only() {
        let start = CalDateTime {
            timestamp: "20260303T100000".to_string(),
            timezone: None,
            date_only: false,
        };
        assert_eq!(format_time_range(&start, &None, false), "10:00");
    }
}
