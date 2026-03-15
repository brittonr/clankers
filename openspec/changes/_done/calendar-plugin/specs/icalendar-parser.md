# iCalendar Parser — VEVENT Extraction and Generation

## Purpose

The iCalendar parser handles conversion between RFC 5545 iCalendar text
format and the plugin's internal `Event` struct. It MUST parse VEVENT
components from VCALENDAR bodies and MUST generate valid VCALENDAR text
for event creation/update.

## Event Data Model

```rust
struct Event {
    /// Unique identifier (UID property)
    uid: String,
    /// Event summary/title (SUMMARY property)
    summary: String,
    /// Start time (DTSTART property)
    start: DateTime,
    /// End time (DTEND property) or computed from DTSTART + DURATION
    end: Option<DateTime>,
    /// Duration (DURATION property, alternative to DTEND)
    duration: Option<Duration>,
    /// Location (LOCATION property)
    location: Option<String>,
    /// Description/notes (DESCRIPTION property)
    description: Option<String>,
    /// Attendee email addresses (ATTENDEE properties)
    attendees: Vec<String>,
    /// Calendar this event belongs to
    calendar: String,
    /// Server-side ETag for conditional updates
    etag: Option<String>,
    /// Resource URL on the CalDAV server
    href: Option<String>,
    /// Whether this is an all-day event
    all_day: bool,
    /// Event status (CONFIRMED, TENTATIVE, CANCELLED)
    status: Option<String>,
}

struct DateTime {
    /// ISO 8601 timestamp
    timestamp: String,
    /// IANA timezone name (e.g., "America/New_York")
    timezone: Option<String>,
    /// Whether this is a date-only value (all-day event)
    date_only: bool,
}
```

## Parsing (iCalendar → Event)

### VEVENT extraction

The parser MUST handle the following VEVENT properties:

| Property | Required | Mapping |
|----------|----------|---------|
| `UID` | MUST | `event.uid` |
| `SUMMARY` | MUST | `event.summary` |
| `DTSTART` | MUST | `event.start` |
| `DTEND` | SHOULD | `event.end` |
| `DURATION` | SHOULD | `event.duration` (when DTEND absent) |
| `LOCATION` | MAY | `event.location` |
| `DESCRIPTION` | MAY | `event.description` |
| `ATTENDEE` | MAY | `event.attendees` (extract email from `mailto:`) |
| `STATUS` | MAY | `event.status` |

### Date/time parsing

The parser MUST handle these DTSTART/DTEND formats:

1. **UTC**: `DTSTART:20260304T140000Z`
2. **With timezone**: `DTSTART;TZID=America/New_York:20260304T140000`
3. **Date-only (all-day)**: `DTSTART;VALUE=DATE:20260304`
4. **Floating (no timezone)**: `DTSTART:20260304T140000` — treat as
   default timezone from config

GIVEN a VCALENDAR body with one VEVENT
WHEN the parser extracts events
THEN it returns exactly one Event with all mapped fields

GIVEN a VCALENDAR body with multiple VEVENTs
WHEN the parser extracts events
THEN it returns all events in document order

GIVEN a VEVENT with DURATION instead of DTEND
WHEN the parser extracts the event
THEN it MUST compute `end` from `start + duration`

GIVEN a VEVENT with VALUE=DATE (all-day event)
WHEN the parser extracts the event
THEN `event.all_day` MUST be `true`
AND `event.start.date_only` MUST be `true`

### Content line unfolding

The parser MUST unfold long lines per RFC 5545 §3.1:
lines beginning with a space or tab are continuations of the previous line.

GIVEN an iCalendar body with folded lines (CRLF + space)
WHEN the parser reads properties
THEN folded lines MUST be joined before property parsing

### Escaped characters

The parser MUST handle escaped characters in text values:
- `\\` → `\`
- `\n` or `\N` → newline
- `\,` → `,`
- `\;` → `;`

## Generation (Event → iCalendar)

### VCALENDAR output

The generator MUST produce valid RFC 5545 VCALENDAR text.

```
BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//clankers//calendar-plugin//EN
BEGIN:VEVENT
UID:{uid}
DTSTAMP:{now_utc}
DTSTART;TZID={tz}:{start}
DTEND;TZID={tz}:{end}
SUMMARY:{summary}
LOCATION:{location}
DESCRIPTION:{description}
ATTENDEE;CN={name}:mailto:{email}
STATUS:{status}
END:VEVENT
END:VCALENDAR
```

The generator MUST:
- Set `DTSTAMP` to current UTC time
- Set `PRODID` to `-//clankers//calendar-plugin//EN`
- Include `VERSION:2.0`
- Use `CRLF` line endings
- Fold lines longer than 75 octets

GIVEN an Event struct with all fields populated
WHEN the generator produces iCalendar text
THEN the output MUST be parseable by the parser (roundtrip)

GIVEN an Event with only required fields (uid, summary, start)
WHEN the generator produces iCalendar text
THEN optional properties MUST be omitted (not empty)

## Duration Parsing

The parser MUST handle ISO 8601 duration format as used in iCalendar:
- `PT1H` → 1 hour
- `PT30M` → 30 minutes
- `PT1H30M` → 1 hour 30 minutes
- `P1D` → 1 day
- `P1DT2H` → 1 day 2 hours

The generator SHOULD accept human-friendly duration strings from the agent
and convert them:
- `"2h"` → `PT2H`
- `"30m"` → `PT30M`
- `"1h30m"` → `PT1H30M`
- `"1d"` → `P1D`
