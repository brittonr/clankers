//! CalDAV HTTP client.
//!
//! Speaks CalDAV (HTTP + WebDAV + iCalendar) to any standards-compliant
//! calendar server. Uses `clankers_plugin_sdk::http` for HTTPS requests.

use std::cell::RefCell;
use std::collections::BTreeMap;

use clankers_plugin_sdk::http;
use serde::Serialize;

use crate::icalendar;

// ═══════════════════════════════════════════════════════════════════════
//  Types
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct CalDavConfig {
    pub url: String,
    pub username: String,
    pub password: String,
    pub default_timezone: String,
    pub default_calendar: Option<String>,
    pub allowed_attendees: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CalendarInfo {
    pub name: String,
    pub url: String,
    pub color: Option<String>,
}

impl CalDavConfig {
    /// Load configuration from Extism plugin config keys.
    pub fn from_plugin_config() -> Result<Self, String> {
        let url = require_config("caldav_url")?;
        let username = require_config("caldav_username")?;
        let password = require_config("caldav_password")?;
        let default_timezone = get_config("default_timezone").unwrap_or_else(|| "UTC".to_string());
        let default_calendar = get_config("default_calendar");
        let allowed_attendees = get_config("allowed_attendees");

        // Reject non-HTTPS URLs — credentials are sent via Basic auth
        if !url.starts_with("https://") {
            return Err(
                "CALDAV_URL must use HTTPS — credentials are sent via Basic auth and \
                 would be exposed in cleartext over HTTP."
                    .to_string(),
            );
        }

        // Ensure URL ends with /
        let url = if url.ends_with('/') {
            url
        } else {
            format!("{url}/")
        };

        Ok(Self {
            url,
            username,
            password,
            default_timezone,
            default_calendar,
            allowed_attendees,
        })
    }
}

fn require_config(key: &str) -> Result<String, String> {
    extism_pdk::config::get(key)
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Missing config: '{key}'. Set the corresponding env var."))
}

fn get_config(key: &str) -> Option<String> {
    extism_pdk::config::get(key)
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
}

// ═══════════════════════════════════════════════════════════════════════
//  Calendar URL cache — avoids PROPFIND on every tool call
// ═══════════════════════════════════════════════════════════════════════

thread_local! {
    static CALENDAR_CACHE: RefCell<Option<Vec<CalendarInfo>>> = RefCell::new(None);
}

fn get_cached_calendars() -> Option<Vec<CalendarInfo>> {
    CALENDAR_CACHE.with(|c| c.borrow().clone())
}

fn set_cached_calendars(calendars: Vec<CalendarInfo>) {
    CALENDAR_CACHE.with(|c| {
        *c.borrow_mut() = Some(calendars);
    });
}

// ═══════════════════════════════════════════════════════════════════════
//  Auth and headers
// ═══════════════════════════════════════════════════════════════════════

/// Generate HTTP Basic auth header value.
pub fn auth_header(username: &str, password: &str) -> String {
    let input = format!("{username}:{password}");
    let encoded = base64_encode(input.as_bytes());
    format!("Basic {encoded}")
}

/// Build common CalDAV request headers.
fn make_headers(config: &CalDavConfig) -> BTreeMap<String, String> {
    let mut headers = BTreeMap::new();
    headers.insert(
        "Authorization".to_string(),
        auth_header(&config.username, &config.password),
    );
    headers.insert("Content-Type".to_string(), "application/xml; charset=utf-8".to_string());
    headers
}

// ═══════════════════════════════════════════════════════════════════════
//  CalDAV operations
// ═══════════════════════════════════════════════════════════════════════

/// Discover calendars via PROPFIND. Results are cached for the plugin lifetime.
pub fn discover_calendars(config: &CalDavConfig) -> Result<Vec<CalendarInfo>, String> {
    if let Some(cached) = get_cached_calendars() {
        return Ok(cached);
    }

    let mut headers = make_headers(config);
    headers.insert("Depth".to_string(), "1".to_string());

    let body = r#"<?xml version="1.0" encoding="UTF-8"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav" xmlns:A="http://apple.com/ns/ical/">
  <D:prop>
    <D:displayname/>
    <D:resourcetype/>
    <A:calendar-color/>
  </D:prop>
</D:propfind>"#;

    let resp = http::request("PROPFIND", &config.url, &headers, Some(body))
        .map_err(|e| format!("Calendar discovery failed: {e}"))?;

    if !resp.is_success() && resp.status != 207 {
        return Err(http_status_to_error(resp.status, "Calendar discovery"));
    }

    let calendars = parse_multistatus_calendars(&resp.text());
    set_cached_calendars(calendars.clone());
    Ok(calendars)
}

/// Query events in a date range via REPORT calendar-query.
pub fn query_events(
    config: &CalDavConfig,
    calendar_url: &str,
    start: &str,
    end: &str,
) -> Result<Vec<icalendar::Event>, String> {
    let mut headers = make_headers(config);
    headers.insert("Depth".to_string(), "1".to_string());

    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<C:calendar-query xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:getetag/>
    <C:calendar-data/>
  </D:prop>
  <C:filter>
    <C:comp-filter name="VCALENDAR">
      <C:comp-filter name="VEVENT">
        <C:time-range start="{start}" end="{end}"/>
      </C:comp-filter>
    </C:comp-filter>
  </C:filter>
</C:calendar-query>"#
    );

    let resp = http::request("REPORT", calendar_url, &headers, Some(&body))
        .map_err(|e| format!("Event query failed: {e}"))?;

    if !resp.is_success() && resp.status != 207 {
        return Err(http_status_to_error(resp.status, "Event query"));
    }

    let entries = parse_multistatus_events(&resp.text());
    let mut events = Vec::new();

    for (href, etag, cal_data) in entries {
        let cal_data = xml_unescape(&cal_data);
        let mut parsed = icalendar::parse_events(&cal_data);
        for event in &mut parsed {
            event.etag = Some(etag.clone());
            event.href = Some(href.clone());
        }
        events.extend(parsed);
    }

    Ok(events)
}

/// Create a new event via PUT with If-None-Match: *.
pub fn create_event(
    config: &CalDavConfig,
    calendar_url: &str,
    ical_body: &str,
    uid: &str,
) -> Result<String, String> {
    let mut headers = make_headers(config);
    headers.insert(
        "Content-Type".to_string(),
        "text/calendar; charset=utf-8".to_string(),
    );
    headers.insert("If-None-Match".to_string(), "*".to_string());

    let base = if calendar_url.ends_with('/') {
        calendar_url.to_string()
    } else {
        format!("{calendar_url}/")
    };
    let url = format!("{base}{uid}.ics");

    let resp = http::request("PUT", &url, &headers, Some(ical_body))
        .map_err(|e| format!("Create event failed: {e}"))?;

    if resp.status == 201 || resp.status == 204 {
        Ok(uid.to_string())
    } else {
        Err(http_status_to_error(resp.status, "Create event"))
    }
}

/// Update an existing event via PUT with If-Match.
pub fn update_event(
    config: &CalDavConfig,
    event_url: &str,
    ical_body: &str,
    etag: &str,
) -> Result<String, String> {
    let mut headers = make_headers(config);
    headers.insert(
        "Content-Type".to_string(),
        "text/calendar; charset=utf-8".to_string(),
    );
    headers.insert("If-Match".to_string(), etag.to_string());

    let resp = http::request("PUT", event_url, &headers, Some(ical_body))
        .map_err(|e| format!("Update event failed: {e}"))?;

    if resp.status == 204 || resp.status == 200 {
        Ok(String::new())
    } else {
        Err(http_status_to_error(resp.status, "Update event"))
    }
}

/// Delete an event via HTTP DELETE.
pub fn delete_event(config: &CalDavConfig, event_url: &str) -> Result<(), String> {
    let headers = make_headers(config);

    let resp = http::request("DELETE", event_url, &headers, None)
        .map_err(|e| format!("Delete event failed: {e}"))?;

    if resp.status == 204 || resp.status == 200 {
        Ok(())
    } else {
        Err(http_status_to_error(resp.status, "Delete event"))
    }
}

/// Fetch a single event by URL. Returns (ical_body, etag).
///
/// Uses a PROPFIND to get both the calendar data and the ETag in one
/// request, since the SDK's HTTP response doesn't expose headers.
pub fn fetch_event(config: &CalDavConfig, event_url: &str) -> Result<(String, String), String> {
    let mut headers = make_headers(config);
    headers.insert("Depth".to_string(), "0".to_string());

    let body = r#"<?xml version="1.0" encoding="UTF-8"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:getetag/>
    <C:calendar-data/>
  </D:prop>
</D:propfind>"#;

    let resp = http::request("PROPFIND", event_url, &headers, Some(body))
        .map_err(|e| format!("Fetch event failed: {e}"))?;

    if !resp.is_success() && resp.status != 207 {
        return Err(http_status_to_error(resp.status, "Fetch event"));
    }

    let entries = parse_multistatus_events(&resp.text());
    let (_href, etag, cal_data) = entries
        .into_iter()
        .next()
        .ok_or("Server returned no event data for PROPFIND.")?;

    let cal_data = xml_unescape(&cal_data);
    Ok((cal_data, etag))
}

/// Resolve a calendar URL by name, or use default.
pub fn resolve_calendar_url(
    config: &CalDavConfig,
    calendar_name: Option<&str>,
) -> Result<String, String> {
    let calendars = discover_calendars(config)?;

    if calendars.is_empty() {
        return Err("No calendars found on the CalDAV server.".to_string());
    }

    let target_name = calendar_name
        .or(config.default_calendar.as_deref());

    if let Some(name) = target_name {
        let name_lower = name.to_lowercase();
        for cal in &calendars {
            if cal.name.to_lowercase() == name_lower {
                return Ok(cal.url.clone());
            }
        }
        return Err(format!(
            "Calendar '{}' not found. Available: {}",
            name,
            calendars
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    // No name specified — use first calendar
    Ok(calendars[0].url.clone())
}

// ═══════════════════════════════════════════════════════════════════════
//  XML parsing (minimal, string-scanning)
// ═══════════════════════════════════════════════════════════════════════

/// Extract (href, etag, calendar-data) from WebDAV multistatus XML.
pub fn parse_multistatus_events(xml: &str) -> Vec<(String, String, String)> {
    let mut results = Vec::new();

    let response_tags = ["<d:response", "<D:response", "<response"];
    let mut remaining = xml;

    while !remaining.is_empty() {
        let start = response_tags
            .iter()
            .filter_map(|tag| remaining.find(tag))
            .min();

        let start = match start {
            Some(s) => s,
            None => break,
        };

        remaining = &remaining[start..];

        let end_tags = ["</d:response>", "</D:response>", "</response>"];
        let end = end_tags
            .iter()
            .filter_map(|tag| remaining.find(tag).map(|p| p + tag.len()))
            .min()
            .unwrap_or(remaining.len());

        let block = &remaining[..end];
        remaining = &remaining[end..];

        let href = extract_tag_content(block, &["d:href", "D:href", "href"]).unwrap_or_default();
        let etag =
            extract_tag_content(block, &["d:getetag", "D:getetag", "getetag"]).unwrap_or_default();
        let cal_data = extract_tag_content(
            block,
            &[
                "cal:calendar-data",
                "C:calendar-data",
                "c:calendar-data",
                "calendar-data",
            ],
        )
        .unwrap_or_default();

        if !cal_data.is_empty() {
            results.push((href, etag.trim_matches('"').to_string(), cal_data));
        }
    }

    results
}

/// Extract calendar names and URLs from PROPFIND multistatus response.
pub fn parse_multistatus_calendars(xml: &str) -> Vec<CalendarInfo> {
    let mut calendars = Vec::new();

    let response_tags = ["<d:response", "<D:response", "<response"];
    let calendar_tags = [
        "<cal:calendar/>",
        "<cal:calendar />",
        "<C:calendar/>",
        "<C:calendar />",
    ];
    let mut remaining = xml;

    while !remaining.is_empty() {
        let start = response_tags
            .iter()
            .filter_map(|tag| remaining.find(tag))
            .min();

        let start = match start {
            Some(s) => s,
            None => break,
        };

        remaining = &remaining[start..];

        let end_tags = ["</d:response>", "</D:response>", "</response>"];
        let end = end_tags
            .iter()
            .filter_map(|tag| remaining.find(tag).map(|p| p + tag.len()))
            .min()
            .unwrap_or(remaining.len());

        let block = &remaining[..end];
        remaining = &remaining[end..];

        let is_calendar = calendar_tags.iter().any(|tag| block.contains(tag));
        if !is_calendar {
            continue;
        }

        let href = extract_tag_content(block, &["d:href", "D:href", "href"]).unwrap_or_default();
        let name = extract_tag_content(block, &["d:displayname", "D:displayname", "displayname"])
            .unwrap_or_default();
        let color = extract_tag_content(
            block,
            &[
                "A:calendar-color",
                "apple:calendar-color",
                "calendar-color",
            ],
        );

        if !href.is_empty() {
            let display_name = if name.is_empty() {
                href.trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or("default")
                    .to_string()
            } else {
                name
            };

            calendars.push(CalendarInfo {
                name: display_name,
                url: href,
                color,
            });
        }
    }

    calendars
}

/// Extract text content between XML tags. Tries multiple tag name variants.
fn extract_tag_content(xml: &str, tag_names: &[&str]) -> Option<String> {
    for tag in tag_names {
        let open = format!("<{}>", tag);
        let close = format!("</{}>", tag);

        if let Some(start) = xml.find(&open) {
            let content_start = start + open.len();
            if let Some(end) = xml[content_start..].find(&close) {
                let content = xml[content_start..content_start + end].trim().to_string();
                return Some(content);
            }
        }

        // Try with attributes: <tag attr="...">content</tag>
        let open_prefix = format!("<{} ", tag);
        if let Some(start) = xml.find(&open_prefix) {
            if let Some(close_bracket) = xml[start..].find('>') {
                let content_start = start + close_bracket + 1;
                if let Some(end) = xml[content_start..].find(&close) {
                    let content = xml[content_start..content_start + end].trim().to_string();
                    return Some(content);
                }
            }
        }
    }
    None
}

/// Unescape XML entities.
pub fn xml_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

// ═══════════════════════════════════════════════════════════════════════
//  Error mapping
// ═══════════════════════════════════════════════════════════════════════

/// Map HTTP status codes to user-friendly error messages.
pub fn http_status_to_error(status: u16, context: &str) -> String {
    let detail = match status {
        401 => "Invalid credentials. Check CALDAV_USERNAME and CALDAV_PASSWORD.",
        403 => "Permission denied. Your account may not have calendar access.",
        404 => "Not found. Check CALDAV_URL is correct.",
        405 => "Method not allowed. The server may not support CalDAV.",
        409 => "Conflict. The resource was modified by another client.",
        412 => "Conflict: event was modified since last fetch. Re-fetch and retry.",
        423 => "Resource is locked. Try again later.",
        500 => "Internal server error. The CalDAV server encountered an error.",
        502 => "Bad gateway. The CalDAV server may be temporarily unavailable.",
        503 => "Service unavailable. The CalDAV server is overloaded or down.",
        _ => "Unexpected error.",
    };
    format!("{context} failed (HTTP {status}): {detail}")
}

// ═══════════════════════════════════════════════════════════════════════
//  Base64 encoding (no external crate)
// ═══════════════════════════════════════════════════════════════════════

const B64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(input: &[u8]) -> String {
    let mut result = String::with_capacity((input.len() + 2) / 3 * 4);
    let chunks = input.chunks(3);

    for chunk in chunks {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(B64_ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        result.push(B64_ALPHABET[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(B64_ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(B64_ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

// ═══════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── auth_header ─────────────────────────────────────────────────

    #[test]
    fn auth_header_basic() {
        assert_eq!(auth_header("user", "pass"), "Basic dXNlcjpwYXNz");
    }

    #[test]
    fn auth_header_special_chars() {
        let header = auth_header("me@fastmail.com", "s3cr3t!@#");
        assert!(header.starts_with("Basic "));
        assert_eq!(
            header,
            format!("Basic {}", base64_encode(b"me@fastmail.com:s3cr3t!@#"))
        );
    }

    #[test]
    fn auth_header_empty_password() {
        assert_eq!(auth_header("user", ""), "Basic dXNlcjo=");
    }

    // ── base64_encode ───────────────────────────────────────────────

    #[test]
    fn base64_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    // ── parse_multistatus_events ────────────────────────────────────

    #[test]
    fn parse_multistatus_basic() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav">
  <d:response>
    <d:href>/cal/personal/event1.ics</d:href>
    <d:propstat>
      <d:prop>
        <d:getetag>"abc123"</d:getetag>
        <cal:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:event1
SUMMARY:Test Event
DTSTART:20260303T100000Z
END:VEVENT
END:VCALENDAR</cal:calendar-data>
      </d:prop>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

        let results = parse_multistatus_events(xml);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "/cal/personal/event1.ics");
        assert_eq!(results[0].1, "abc123");
        assert!(results[0].2.contains("BEGIN:VEVENT"));
    }

    #[test]
    fn parse_multistatus_empty() {
        let xml = r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:"></d:multistatus>"#;
        let results = parse_multistatus_events(xml);
        assert!(results.is_empty());
    }

    #[test]
    fn parse_multistatus_xml_entities() {
        let xml = r#"<d:multistatus xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav">
  <d:response>
    <d:href>/cal/test.ics</d:href>
    <d:propstat>
      <d:prop>
        <d:getetag>"etag1"</d:getetag>
        <cal:calendar-data>BEGIN:VCALENDAR
BEGIN:VEVENT
UID:ent-1
SUMMARY:A &amp; B
DTSTART:20260303T100000Z
END:VEVENT
END:VCALENDAR</cal:calendar-data>
      </d:prop>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

        let results = parse_multistatus_events(xml);
        assert_eq!(results.len(), 1);
        assert!(results[0].2.contains("&amp;") || results[0].2.contains("& B"));
    }

    // ── parse_multistatus_calendars ─────────────────────────────────

    #[test]
    fn parse_calendars_propfind() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav" xmlns:A="http://apple.com/ns/ical/">
  <d:response>
    <d:href>/dav/calendars/user/me/</d:href>
    <d:propstat>
      <d:prop>
        <d:resourcetype><d:collection/></d:resourcetype>
        <d:displayname>Calendars</d:displayname>
      </d:prop>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/dav/calendars/user/me/personal/</d:href>
    <d:propstat>
      <d:prop>
        <d:resourcetype><d:collection/><cal:calendar/></d:resourcetype>
        <d:displayname>Personal</d:displayname>
        <A:calendar-color>#0000FF</A:calendar-color>
      </d:prop>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/dav/calendars/user/me/work/</d:href>
    <d:propstat>
      <d:prop>
        <d:resourcetype><d:collection/><cal:calendar/></d:resourcetype>
        <d:displayname>Work</d:displayname>
      </d:prop>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

        let calendars = parse_multistatus_calendars(xml);
        assert_eq!(calendars.len(), 2);
        assert_eq!(calendars[0].name, "Personal");
        assert_eq!(calendars[0].url, "/dav/calendars/user/me/personal/");
        assert_eq!(calendars[0].color, Some("#0000FF".to_string()));
        assert_eq!(calendars[1].name, "Work");
    }

    // ── xml_unescape ────────────────────────────────────────────────

    #[test]
    fn xml_unescape_all_entities() {
        assert_eq!(xml_unescape("a &amp; b"), "a & b");
        assert_eq!(xml_unescape("a &lt; b"), "a < b");
        assert_eq!(xml_unescape("a &gt; b"), "a > b");
        assert_eq!(xml_unescape("&quot;hello&quot;"), "\"hello\"");
        assert_eq!(xml_unescape("it&apos;s"), "it's");
    }

    #[test]
    fn xml_unescape_no_entities() {
        assert_eq!(xml_unescape("plain text"), "plain text");
    }

    // ── http_status_to_error ────────────────────────────────────────

    #[test]
    fn error_messages() {
        let e = http_status_to_error(401, "Test");
        assert!(e.contains("Invalid credentials"));
        assert!(e.contains("401"));

        let e = http_status_to_error(404, "Test");
        assert!(e.contains("Not found"));

        let e = http_status_to_error(412, "Test");
        assert!(e.contains("Conflict"));

        let e = http_status_to_error(503, "Test");
        assert!(e.contains("unavailable"));
    }
}
