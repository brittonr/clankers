//! clankers-calendar — CalDAV calendar integration.
//!
//! Lists, creates, updates, and deletes events on any CalDAV server
//! (Fastmail, Google, Nextcloud, Radicale, etc.).
//!
//! ## Setup
//!
//! Set environment variables:
//! ```
//! export CALDAV_URL=https://caldav.fastmail.com/dav/calendars/user/me@fastmail.com/
//! export CALDAV_USERNAME=me@fastmail.com
//! export CALDAV_PASSWORD=app-specific-password
//! export CLANKERS_TIMEZONE=America/New_York    # optional, defaults to UTC
//! export CALDAV_DEFAULT_CALENDAR=personal      # optional
//! export CALDAV_ALLOWED_ATTENDEES=*@mycompany.com,alice@example.com  # optional
//! ```
//!
//! ## Tools
//!
//! - **`list_events`** — List events in a date range
//! - **`create_event`** — Create a new event
//! - **`update_event`** — Modify an existing event
//! - **`delete_event`** — Remove an event
//! - **`check_availability`** — Check free/busy for a time range

mod cache;
mod caldav;
mod icalendar;

use clankers_plugin_sdk::prelude::*;
use clankers_plugin_sdk::serde_json;

// ═══════════════════════════════════════════════════════════════════════
//  Extism guest entrypoints
// ═══════════════════════════════════════════════════════════════════════

#[plugin_fn]
pub fn handle_tool_call(input: String) -> FnResult<String> {
    dispatch_tools(&input, &[
        ("list_events", handle_list_events),
        ("create_event", handle_create_event),
        ("update_event", handle_update_event),
        ("delete_event", handle_delete_event),
        ("check_availability", handle_check_availability),
    ])
}

#[plugin_fn]
pub fn on_event(input: String) -> FnResult<String> {
    let evt: serde_json::Value =
        serde_json::from_str(&input).map_err(|e| Error::msg(format!("Invalid event JSON: {e}")))?;

    let event_name = evt
        .get("event")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let response = match event_name {
        "agent_start" => handle_agent_start(),
        "turn_start" => handle_turn_start(),
        _ => serde_json::json!({
            "event": event_name,
            "handled": false,
            "message": format!("Unhandled event: {event_name}")
        }),
    };

    Ok(serde_json::to_string(&response)?)
}

#[plugin_fn]
pub fn describe(Json(_): Json<()>) -> FnResult<Json<PluginMeta>> {
    Ok(Json(PluginMeta::new(
        "clankers-calendar",
        "0.1.0",
        &[
            ("list_events", "List calendar events in a date range"),
            ("create_event", "Create a new calendar event"),
            ("update_event", "Update an existing calendar event"),
            ("delete_event", "Delete a calendar event"),
            ("check_availability", "Check free/busy for a time range"),
        ],
        &[],
    )))
}

// ═══════════════════════════════════════════════════════════════════════
//  Time helpers — reads host-injected config
// ═══════════════════════════════════════════════════════════════════════

/// Read the host-injected current UTC time (format: YYYYMMDDTHHMMSSZ).
fn get_current_time() -> String {
    extism_pdk::config::get("current_time")
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "20260101T000000Z".to_string())
}

/// Read the host-injected current Unix timestamp.
fn get_current_unix() -> u64 {
    extism_pdk::config::get("current_time_unix")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0)
}

/// Get the YYYYMMDD date portion from a CalDAV timestamp.
fn date_part(ts: &str) -> &str {
    if let Some(t_pos) = ts.find('T') {
        &ts[..t_pos]
    } else {
        &ts[..8.min(ts.len())]
    }
}

/// Get current time as DTSTAMP value.
fn get_now_dtstamp() -> String {
    get_current_time()
}

/// Get today at 00:00Z.
fn get_today_start() -> String {
    let now = get_current_time();
    let date = date_part(&now);
    format!("{date}T000000Z")
}

/// Get today at 23:59:59Z.
fn get_today_end() -> String {
    let now = get_current_time();
    let date = date_part(&now);
    format!("{date}T235959Z")
}

/// Add hours to the current time.
fn add_hours_to_now(hours: u32) -> String {
    let now = get_current_time();
    let dt = icalendar::CalDateTime {
        timestamp: now.trim_end_matches('Z').to_string(),
        timezone: Some("UTC".to_string()),
        date_only: false,
    };
    let result = icalendar::add_minutes_to_datetime(&dt, u64::from(hours) * 60);
    format!("{}Z", result.timestamp)
}

/// Subtract days from today.
fn subtract_days_from_today(days: u32) -> String {
    let now = get_current_time();
    let date = date_part(&now);

    let year: u32 = date.get(0..4).and_then(|s| s.parse().ok()).unwrap_or(2026);
    let month: u32 = date.get(4..6).and_then(|s| s.parse().ok()).unwrap_or(1);
    let day: u32 = date.get(6..8).and_then(|s| s.parse().ok()).unwrap_or(1);

    let (y, m, d) = subtract_days(year, month, day, days);
    format!("{y:04}{m:02}{d:02}T000000Z")
}

/// Add days from today.
fn add_days_from_today(days: u32) -> String {
    let now = get_current_time();
    let date = date_part(&now);

    let year: u32 = date.get(0..4).and_then(|s| s.parse().ok()).unwrap_or(2026);
    let month: u32 = date.get(4..6).and_then(|s| s.parse().ok()).unwrap_or(1);
    let day: u32 = date.get(6..8).and_then(|s| s.parse().ok()).unwrap_or(1);

    let (y, m, d) = icalendar::add_days(year, month, day, days);
    format!("{y:04}{m:02}{d:02}T235959Z")
}

fn subtract_days(mut year: u32, mut month: u32, mut day: u32, mut n: u32) -> (u32, u32, u32) {
    while n > 0 {
        if day > n {
            day -= n;
            n = 0;
        } else {
            n -= day;
            month -= 1;
            if month == 0 {
                month = 12;
                year -= 1;
            }
            day = icalendar::days_in_month(year, month);
        }
    }
    (year, month, day)
}

// ═══════════════════════════════════════════════════════════════════════
//  Tool handlers
// ═══════════════════════════════════════════════════════════════════════

fn handle_list_events(args: &Value) -> Result<String, String> {
    let config = caldav::CalDavConfig::from_plugin_config()?;

    let start = args.get_str("start").unwrap_or("");
    let end = args.get_str("end").unwrap_or("");
    let calendar_name = args.get_str("calendar");

    let start_caldav = if start.is_empty() {
        get_today_start()
    } else {
        normalize_datetime_input(start)
    };

    let end_caldav = if end.is_empty() {
        get_today_end()
    } else {
        normalize_datetime_input(end)
    };

    let calendar_url = caldav::resolve_calendar_url(&config, calendar_name)?;
    let events = caldav::query_events(&config, &calendar_url, &start_caldav, &end_caldav)?;

    if events.is_empty() {
        return Ok("No events found for this period.".to_string());
    }

    let mut sorted = events;
    sorted.sort_by(|a, b| a.start.timestamp.cmp(&b.start.timestamp));

    let mut lines = vec![format!("Found {} event(s):\n", sorted.len())];
    for event in &sorted {
        lines.push(format_event_line(event));
    }

    Ok(lines.join("\n"))
}

fn handle_create_event(args: &Value) -> Result<String, String> {
    let config = caldav::CalDavConfig::from_plugin_config()?;

    let summary = args.require_str("summary")?;
    let start_input = args.require_str("start")?;
    let end_input = args.get_str("end");
    let duration_input = args.get_str("duration");
    let location = args.get_str("location").map(String::from);
    let description = args.get_str("description").map(String::from);
    let attendees_str = args.get_str("attendees");
    let calendar_name = args.get_str("calendar");
    let all_day = args.get_bool_or("all_day", false);

    // Parse start
    let start_ts = normalize_datetime_input(start_input);
    let start = icalendar::CalDateTime {
        timestamp: start_ts.trim_end_matches('Z').to_string(),
        timezone: if all_day {
            None
        } else {
            Some(config.default_timezone.clone())
        },
        date_only: all_day,
    };

    // Determine end
    let end = if let Some(end_str) = end_input {
        let end_ts = normalize_datetime_input(end_str);
        Some(icalendar::CalDateTime {
            timestamp: end_ts.trim_end_matches('Z').to_string(),
            timezone: start.timezone.clone(),
            date_only: all_day,
        })
    } else if let Some(dur_str) = duration_input {
        let iso_dur = icalendar::parse_human_duration(dur_str)
            .unwrap_or_else(|| dur_str.to_string());
        if let Some(mins) = icalendar::duration_to_minutes(&iso_dur) {
            Some(icalendar::add_minutes_to_datetime(&start, mins))
        } else {
            None
        }
    } else {
        // Default: 1 hour
        Some(icalendar::add_minutes_to_datetime(&start, 60))
    };

    // Parse and validate attendees
    let attendees: Vec<String> = attendees_str
        .map(|s| {
            s.split(',')
                .map(|a| a.trim().to_string())
                .filter(|a| !a.is_empty())
                .collect()
        })
        .unwrap_or_default();

    if !attendees.is_empty() {
        let allowed = config.allowed_attendees.as_deref().unwrap_or("");
        for attendee in &attendees {
            if !is_attendee_allowed(attendee, allowed) {
                return Err(format!(
                    "Attendee '{}' is not in the allowed list. \
                     Set CALDAV_ALLOWED_ATTENDEES to permit this address.",
                    attendee
                ));
            }
        }
    }

    // Generate UID with timestamp for uniqueness
    let now = get_current_time();
    let uid = generate_uid(&format!("{summary}-{start_input}-{now}"));

    let event = icalendar::Event {
        uid: uid.clone(),
        summary: summary.to_string(),
        start,
        end,
        duration: None,
        location,
        description,
        attendees,
        calendar: String::new(),
        etag: None,
        href: None,
        all_day,
        status: Some("CONFIRMED".to_string()),
    };

    let dtstamp = get_now_dtstamp();
    let ical_body = icalendar::generate_vcalendar(&event, &dtstamp);

    let calendar_url = caldav::resolve_calendar_url(&config, calendar_name)?;
    caldav::create_event(&config, &calendar_url, &ical_body, &uid)?;

    cache::invalidate();

    let time_range = icalendar::format_time_range(&event.start, &event.end, all_day);
    Ok(format!("Created event '{}' — {}\nUID: {}", summary, time_range, uid))
}

fn handle_update_event(args: &Value) -> Result<String, String> {
    let config = caldav::CalDavConfig::from_plugin_config()?;
    let uid = args.require_str("uid")?;

    // Find the event by querying a wide range
    let calendar_url = caldav::resolve_calendar_url(&config, None)?;
    let start = subtract_days_from_today(30);
    let end = add_days_from_today(365);
    let events = caldav::query_events(&config, &calendar_url, &start, &end)?;

    let target = events
        .iter()
        .find(|e| e.uid == uid)
        .ok_or_else(|| format!("Event not found: {uid}"))?;

    let event_href = target
        .href
        .as_ref()
        .ok_or("Event has no server URL — cannot update.")?;

    // Fetch current version with ETag
    let (current_ical, current_etag) = caldav::fetch_event(&config, event_href)?;

    if current_etag.is_empty() {
        return Err("Could not retrieve event ETag — cannot safely update. Re-fetch and retry.".to_string());
    }

    let mut parsed = icalendar::parse_events(&current_ical);
    let event = parsed
        .first_mut()
        .ok_or("Failed to parse current event from server.")?;

    // Merge changes
    if let Some(s) = args.get_str("summary") {
        event.summary = s.to_string();
    }
    if let Some(s) = args.get_str("start") {
        event.start.timestamp = normalize_datetime_input(s).trim_end_matches('Z').to_string();
    }
    if let Some(s) = args.get_str("end") {
        event.end = Some(icalendar::CalDateTime {
            timestamp: normalize_datetime_input(s).trim_end_matches('Z').to_string(),
            timezone: event.start.timezone.clone(),
            date_only: event.start.date_only,
        });
    }
    if let Some(dur_str) = args.get_str("duration") {
        let iso_dur = icalendar::parse_human_duration(dur_str)
            .unwrap_or_else(|| dur_str.to_string());
        if let Some(mins) = icalendar::duration_to_minutes(&iso_dur) {
            event.end = Some(icalendar::add_minutes_to_datetime(&event.start, mins));
        }
    }
    if let Some(s) = args.get_str("location") {
        event.location = Some(s.to_string());
    }
    if let Some(s) = args.get_str("description") {
        event.description = Some(s.to_string());
    }

    // Generate updated iCal and PUT back
    let dtstamp = get_now_dtstamp();
    let new_ical = icalendar::generate_vcalendar(event, &dtstamp);
    caldav::update_event(&config, event_href, &new_ical, &current_etag)?;

    cache::invalidate();

    Ok(format!("Updated event '{}'", event.summary))
}

fn handle_delete_event(args: &Value) -> Result<String, String> {
    let config = caldav::CalDavConfig::from_plugin_config()?;
    let uid = args.require_str("uid")?;

    // Find the event
    let calendar_url = caldav::resolve_calendar_url(&config, None)?;
    let start = subtract_days_from_today(30);
    let end = add_days_from_today(365);
    let events = caldav::query_events(&config, &calendar_url, &start, &end)?;

    let target = events
        .iter()
        .find(|e| e.uid == uid)
        .ok_or_else(|| format!("Event not found: {uid}"))?;

    let event_href = target
        .href
        .as_ref()
        .ok_or("Event has no server URL — cannot delete.")?;

    let summary = target.summary.clone();
    caldav::delete_event(&config, event_href)?;

    cache::invalidate();

    Ok(format!("Deleted event '{summary}'"))
}

fn handle_check_availability(args: &Value) -> Result<String, String> {
    let config = caldav::CalDavConfig::from_plugin_config()?;

    let start = args.require_str("start")?;
    let end = args.require_str("end")?;

    let start_caldav = normalize_datetime_input(start);
    let end_caldav = normalize_datetime_input(end);

    let calendar_url = caldav::resolve_calendar_url(&config, None)?;
    let events = caldav::query_events(&config, &calendar_url, &start_caldav, &end_caldav)?;

    if events.is_empty() {
        let start_hm = extract_time_from_input(start);
        let end_hm = extract_time_from_input(end);
        return Ok(format!("You are free from {start_hm} to {end_hm}."));
    }

    let mut sorted = events;
    sorted.sort_by(|a, b| a.start.timestamp.cmp(&b.start.timestamp));

    let mut lines = vec!["Schedule:".to_string()];
    for event in &sorted {
        let time_range = icalendar::format_time_range(&event.start, &event.end, event.all_day);
        lines.push(format!("• {}  BUSY ({})", time_range, event.summary));
    }

    // Compute free gaps
    let range_start = start_caldav.trim_end_matches('Z');
    let range_end = end_caldav.trim_end_matches('Z');

    let mut free_gaps = Vec::new();
    let mut cursor = range_start.to_string();

    for event in &sorted {
        let ev_start = &event.start.timestamp;
        if cursor < *ev_start {
            let start_hm = extract_hm_from_timestamp(&cursor);
            let end_hm = extract_hm_from_timestamp(ev_start);
            free_gaps.push(format!("• {start_hm}–{end_hm}  FREE"));
        }
        if let Some(ref end) = event.end {
            if end.timestamp > cursor {
                cursor = end.timestamp.clone();
            }
        }
    }
    if *cursor < *range_end {
        let start_hm = extract_hm_from_timestamp(&cursor);
        let end_hm = extract_hm_from_timestamp(range_end);
        free_gaps.push(format!("• {start_hm}–{end_hm}  FREE"));
    }

    if !free_gaps.is_empty() {
        lines.push(String::new());
        lines.push("Free slots:".to_string());
        lines.extend(free_gaps);
    }

    Ok(lines.join("\n"))
}

// ═══════════════════════════════════════════════════════════════════════
//  Event handlers
// ═══════════════════════════════════════════════════════════════════════

fn handle_agent_start() -> serde_json::Value {
    let config = match caldav::CalDavConfig::from_plugin_config() {
        Ok(c) => c,
        Err(_) => {
            return serde_json::json!({
                "event": "agent_start",
                "handled": false,
                "message": "Calendar not configured. Set CALDAV_URL, CALDAV_USERNAME, CALDAV_PASSWORD."
            });
        }
    };

    let now_unix = get_current_unix();

    // Query next 8 hours
    let start = get_current_time();
    let end = add_hours_to_now(8);

    let calendar_url = match caldav::resolve_calendar_url(&config, None) {
        Ok(url) => url,
        Err(_) => {
            return serde_json::json!({
                "event": "agent_start",
                "handled": true,
                "display": "📅 Calendar unavailable (could not discover calendars)."
            });
        }
    };

    let events = match caldav::query_events(&config, &calendar_url, &start, &end) {
        Ok(e) => e,
        Err(_) => {
            return serde_json::json!({
                "event": "agent_start",
                "handled": true,
                "display": "📅 Calendar unavailable (connection failed)."
            });
        }
    };

    cache::set_cache(events.clone(), now_unix);

    if events.is_empty() {
        return serde_json::json!({
            "event": "agent_start",
            "handled": true,
            "display": "📅 No upcoming events. Calendar is clear.",
            "ui": [
                {
                    "action": "set_status",
                    "text": "📅 Clear",
                    "color": "green"
                }
            ]
        });
    }

    let mut sorted = events;
    sorted.sort_by(|a, b| a.start.timestamp.cmp(&b.start.timestamp));

    let agenda = format_agenda(&sorted, &config.default_timezone);
    let widget_items = format_widget_items(&sorted);

    let next = &sorted[0];
    let next_text = format!("📅 Next: {}", next.summary);

    serde_json::json!({
        "event": "agent_start",
        "handled": true,
        "display": agenda,
        "ui": [
            {
                "action": "set_status",
                "text": next_text,
                "color": "cyan"
            },
            {
                "action": "set_widget",
                "widget": {
                    "type": "Box",
                    "direction": "Vertical",
                    "children": [
                        { "type": "Text", "content": "📅 Today", "bold": true, "color": "cyan" },
                        { "type": "List", "items": widget_items, "selected": 0 }
                    ]
                }
            }
        ]
    })
}

fn handle_turn_start() -> serde_json::Value {
    let now_unix = get_current_unix();

    let events = match cache::get_cached(now_unix) {
        Some(e) => e,
        None => {
            // Cache expired — try to refresh
            let config = match caldav::CalDavConfig::from_plugin_config() {
                Ok(c) => c,
                Err(_) => {
                    return serde_json::json!({
                        "event": "turn_start",
                        "handled": true
                    });
                }
            };

            let start = get_current_time();
            let end = add_hours_to_now(8);

            let calendar_url = match caldav::resolve_calendar_url(&config, None) {
                Ok(url) => url,
                Err(_) => {
                    return serde_json::json!({
                        "event": "turn_start",
                        "handled": true
                    });
                }
            };

            match caldav::query_events(&config, &calendar_url, &start, &end) {
                Ok(e) => {
                    cache::set_cache(e.clone(), now_unix);
                    e
                }
                Err(_) => {
                    return serde_json::json!({
                        "event": "turn_start",
                        "handled": true
                    });
                }
            }
        }
    };

    if events.is_empty() {
        return serde_json::json!({
            "event": "turn_start",
            "handled": true,
            "ui": {
                "action": "set_status",
                "text": "📅 Clear",
                "color": "green"
            }
        });
    }

    let mut sorted = events;
    sorted.sort_by(|a, b| a.start.timestamp.cmp(&b.start.timestamp));

    let next = &sorted[0];
    let text = format!("📅 Next: {}", next.summary);

    // Color based on urgency
    let now_ts = get_current_time();
    let now_trimmed = now_ts.trim_end_matches('Z');
    let color = if *next.start.timestamp <= *now_trimmed {
        "red" // happening now or overdue
    } else {
        // Check if within ~30 minutes (rough: compare hour+minute digits)
        let mins_until = estimate_minutes_between(now_trimmed, &next.start.timestamp);
        if mins_until <= 15 {
            "yellow"
        } else {
            "cyan"
        }
    };

    serde_json::json!({
        "event": "turn_start",
        "handled": true,
        "ui": {
            "action": "set_status",
            "text": text,
            "color": color
        }
    })
}

// ═══════════════════════════════════════════════════════════════════════
//  Helpers
// ═══════════════════════════════════════════════════════════════════════

/// Generate a UID from a seed string. The seed should include a timestamp
/// component for uniqueness.
fn generate_uid(seed: &str) -> String {
    let mut hash: u64 = 5381;
    for b in seed.as_bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(*b as u64);
    }
    let hash2 = hash.wrapping_mul(2654435761);
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}@clankers",
        (hash & 0xFFFFFFFF) as u32,
        ((hash >> 32) & 0xFFFF) as u16,
        (hash2 & 0x0FFF) as u16,
        ((hash2 >> 12) & 0x3FFF | 0x8000) as u16,
        (hash2 >> 26) & 0xFFFFFFFFFFFF,
    )
}

/// Check if an attendee email is allowed.
fn is_attendee_allowed(email: &str, allowed_patterns: &str) -> bool {
    if allowed_patterns.is_empty() {
        return false;
    }

    let email_lower = email.to_lowercase();
    for pattern in allowed_patterns.split(',') {
        let pattern = pattern.trim().to_lowercase();
        if pattern.is_empty() {
            continue;
        }
        // Exact match
        if pattern == email_lower {
            return true;
        }
        // Domain wildcard: *@domain.com
        if let Some(domain) = pattern.strip_prefix("*@") {
            if let Some(email_domain) = email_lower.split('@').nth(1) {
                if email_domain == domain {
                    return true;
                }
            }
        }
        // Wildcard all
        if pattern == "*" {
            return true;
        }
    }
    false
}

/// Format a single event as a bullet line.
fn format_event_line(event: &icalendar::Event) -> String {
    let time_range = icalendar::format_time_range(&event.start, &event.end, event.all_day);
    let mut line = format!("• {}  {}  [{}]", time_range, event.summary, event.uid);
    if let Some(ref loc) = event.location {
        line.push_str(&format!(" ({loc})"));
    }
    line
}

/// Format full agenda for display.
fn format_agenda(events: &[icalendar::Event], timezone: &str) -> String {
    let mut lines = vec![format!("📅 Today's calendar ({timezone}):")];
    for event in events {
        let time_range = icalendar::format_time_range(&event.start, &event.end, event.all_day);
        let mut line = format!("  {}  {}", time_range, event.summary);
        if let Some(ref loc) = event.location {
            line.push_str(&format!(" ({loc})"));
        }
        lines.push(line);
    }
    lines.join("\n")
}

/// Format event summaries for the TUI widget List items.
fn format_widget_items(events: &[icalendar::Event]) -> Vec<String> {
    events
        .iter()
        .map(|e| {
            let time = icalendar::format_time_range(&e.start, &e.end, e.all_day);
            format!("{time} {}", e.summary)
        })
        .collect()
}

/// Rough estimate of minutes between two YYYYMMDDTHHMMSS timestamps.
fn estimate_minutes_between(from: &str, to: &str) -> u64 {
    let from_mins = parse_timestamp_minutes(from);
    let to_mins = parse_timestamp_minutes(to);
    to_mins.saturating_sub(from_mins)
}

fn parse_timestamp_minutes(ts: &str) -> u64 {
    let t_pos = match ts.find('T') {
        Some(p) => p,
        None => return 0,
    };
    let time = &ts[t_pos + 1..];
    let h: u64 = time.get(0..2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let m: u64 = time.get(2..4).and_then(|s| s.parse().ok()).unwrap_or(0);
    // Include date for cross-day comparison
    let d: u64 = ts.get(6..8).and_then(|s| s.parse().ok()).unwrap_or(0);
    d * 1440 + h * 60 + m
}

// ── Date/time normalization ─────────────────────────────────────────

/// Normalize user input (ISO 8601) to CalDAV format (YYYYMMDDTHHMMSSZ).
fn normalize_datetime_input(input: &str) -> String {
    let s = input.trim();

    // Already in CalDAV format?
    if s.len() >= 15 && s.contains('T') && !s.contains('-') {
        if s.ends_with('Z') {
            return s.to_string();
        }
        return format!("{s}Z");
    }

    // ISO 8601 with dashes: 2026-03-03 or 2026-03-03T14:00
    let stripped = s.replace('-', "").replace(':', "");

    if stripped.len() == 8 {
        // Date only — start of day
        return format!("{stripped}T000000Z");
    }

    if stripped.contains('T') {
        let parts: Vec<&str> = stripped.split('T').collect();
        if parts.len() == 2 {
            let date = parts[0];
            let mut time = parts[1].replace('Z', "");
            while time.len() < 6 {
                time.push('0');
            }
            return format!("{date}T{time}Z");
        }
    }

    format!("{stripped}Z")
}

fn extract_time_from_input(input: &str) -> String {
    let normalized = normalize_datetime_input(input);
    extract_hm_from_timestamp(&normalized)
}

fn extract_hm_from_timestamp(ts: &str) -> String {
    if let Some(t_pos) = ts.find('T') {
        let time_part = &ts[t_pos + 1..];
        let hour = time_part.get(0..2).unwrap_or("00");
        let min = time_part.get(2..4).unwrap_or("00");
        format!("{hour}:{min}")
    } else {
        "00:00".to_string()
    }
}
