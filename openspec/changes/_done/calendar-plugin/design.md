# calendar-plugin — Design

## Decisions

### CalDAV over vendor-specific APIs

**Choice:** CalDAV (RFC 4791) as the sole calendar protocol.
**Rationale:** CalDAV is the universal calendar standard. Every major provider
supports it: Fastmail (natively), Google (via CalDAV endpoint), Apple iCloud,
Nextcloud, Radicale, Baikal, Zimbra. A single CalDAV implementation covers
all these providers. Vendor-specific APIs (Google Calendar REST, Microsoft
Graph) would each require separate HTTP clients, auth flows, and data models.
CalDAV uses standard HTTP methods (PROPFIND, REPORT, PUT, DELETE) with
iCalendar payloads — well-documented and stable.
**Alternatives considered:** Google Calendar API (vendor lock-in), .ics file
import/export only (no live sync), hybrid approach (too complex for v1).

### WASM plugin, not a core feature

**Choice:** Implement as an Extism WASM plugin under `plugins/clankers-calendar/`.
**Rationale:** Follows the same pattern as `clankers-email` and `clankers-hash`.
Calendar is optional functionality — not every user wants it. WASM sandboxing
limits the plugin to only its declared permissions (`net` with CalDAV server
hosts). Plugin can be developed, versioned, and distributed independently.
**Alternatives considered:** Core `src/calendar/` module (too coupled, bloats
binary for non-users), external sidecar process (loses WASM sandboxing and
tool registration).

### iCalendar parsing in WASM via `ical` crate

**Choice:** Use the `ical` crate (or hand-rolled parser for subset) compiled
to `wasm32-unknown-unknown`.
**Rationale:** iCalendar (RFC 5545) is the data format inside CalDAV. Events
are VEVENT components with DTSTART, DTEND, SUMMARY, LOCATION, etc. The
format is text-based and well-specified. A purpose-built minimal parser
for VEVENT extraction keeps the WASM binary small.
**Alternatives considered:** `ical-rs` (feature-heavy, large WASM size),
full iCalendar library (overkill — we only need VEVENT read/write),
serde-based format (CalDAV servers speak iCalendar, not JSON).

### Credentials via `config_env`, same as clankers-email

**Choice:** CalDAV URL and credentials injected via environment variables
mapped through `plugin.json` `config_env`.
**Rationale:** Consistent with the existing auth pattern. `CALDAV_URL`,
`CALDAV_USERNAME`, `CALDAV_PASSWORD` env vars. No OAuth dance needed —
users generate app-specific passwords. Fastmail, Google, and Apple all
support app passwords for CalDAV.
**Alternatives considered:** OAuth2 (complex, requires browser redirect,
bad for headless/daemon), credential file (yet another config location),
interactive prompt (breaks daemon mode).

### Timezone via config, not system detection

**Choice:** User sets `CLANKERS_TIMEZONE` env var (e.g., `America/New_York`).
Default to UTC if unset.
**Rationale:** WASM modules can't read `/etc/localtime`. The host clankers
process could detect it, but Extism config injection is the clean path.
Explicit timezone avoids surprises in remote/daemon scenarios where system
timezone might be UTC regardless of the user's location.
**Alternatives considered:** System timezone detection in host, pass to
plugin (adds host-side code for a plugin feature), always UTC (confusing
for users — "my 2pm meeting shows as 7pm").

### Flat event list, not a calendar grid in TUI

**Choice:** Today's agenda as a flat chronological list in the plugin UI
widget. Next event in the status bar.
**Rationale:** The TUI is a coding tool — screen real estate is precious.
A full calendar grid (month/week view) is too large and rarely useful during
coding. A compact agenda list ("10:00 Standup · 14:00 Deep work · 16:30 1:1")
is scannable at a glance. The status bar segment ("Next: Standup in 23m")
provides ambient awareness without switching panels.
**Alternatives considered:** Full month grid (too large), week view (still
too large), popover on demand (plugin UI doesn't support popovers yet).

## Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                        clankers host                                 │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │                   PluginManager (extism)                      │    │
│  │                                                               │    │
│  │  ┌───────────────────────────────────────────────────────┐   │    │
│  │  │              clankers-calendar.wasm                     │   │    │
│  │  │                                                        │   │    │
│  │  │  ┌──────────────┐  ┌──────────────┐  ┌────────────┐  │   │    │
│  │  │  │ handle_tool_ │  │  on_event    │  │  describe   │  │   │    │
│  │  │  │ call()       │  │  ()          │  │  ()         │  │   │    │
│  │  │  └──────┬───────┘  └──────┬───────┘  └────────────┘  │   │    │
│  │  │         │                  │                           │   │    │
│  │  │         ▼                  ▼                           │   │    │
│  │  │  ┌──────────────────────────────────────────────────┐ │   │    │
│  │  │  │              CalDAV Client                       │ │   │    │
│  │  │  │                                                  │ │   │    │
│  │  │  │  caldav_url       (from config)                  │ │   │    │
│  │  │  │  username         (from config)                  │ │   │    │
│  │  │  │  password         (from config)                  │ │   │    │
│  │  │  │  default_timezone (from config)                  │ │   │    │
│  │  │  │                                                  │ │   │    │
│  │  │  │  PROPFIND → discover calendars                   │ │   │    │
│  │  │  │  REPORT   → query events (calendar-query)        │ │   │    │
│  │  │  │  PUT      → create/update event                  │ │   │    │
│  │  │  │  DELETE   → remove event                         │ │   │    │
│  │  │  └──────────────────────────────────────────────────┘ │   │    │
│  │  │                                                        │   │    │
│  │  │  ┌──────────────────────────────────────────────────┐ │   │    │
│  │  │  │           iCalendar Parser/Generator             │ │   │    │
│  │  │  │                                                  │ │   │    │
│  │  │  │  parse VCALENDAR → Vec<Event>                    │ │   │    │
│  │  │  │  Event → VCALENDAR string                        │ │   │    │
│  │  │  └──────────────────────────────────────────────────┘ │   │    │
│  │  └────────────────────────────────────────────────────────┘  │    │
│  └──────────────────────────────────────────────────────────────┘    │
│                                                                      │
│  Tools exposed to LLM:                                               │
│    list_events · create_event · update_event ·                       │
│    delete_event · check_availability                                 │
│                                                                      │
│  Events subscribed:                                                  │
│    agent_start → inject today's agenda                               │
│    turn_start  → refresh "next event" status bar                     │
│                                                                      │
│  UI:                                                                 │
│    Status bar segment: "Next: Standup in 23m"                        │
│    Widget: Today's agenda list                                       │
└──────────────────────────────────────────────────────────────────────┘
                           │
                           │ HTTP (CalDAV)
                           ▼
              ┌─────────────────────────┐
              │   CalDAV Server         │
              │   (Fastmail / Google /  │
              │    Nextcloud / etc.)    │
              └─────────────────────────┘
```

## Data Flow

### List events
1. Agent calls `list_events` tool with `{ "start": "2026-03-03", "end": "2026-03-04" }`
2. Plugin builds CalDAV REPORT request with `calendar-query` filter
3. HTTP request to CalDAV server with Basic auth
4. Server returns multiget response with VCALENDAR bodies
5. Plugin parses iCalendar, extracts VEVENT components
6. Converts to JSON: `[{ "uid": "...", "summary": "Standup", "start": "10:00", "end": "10:30", "location": "Zoom" }]`
7. Returns formatted event list to agent

### Create event
1. Agent calls `create_event` with `{ "summary": "Deep work", "start": "2026-03-04T14:00", "duration": "2h" }`
2. Plugin generates UUID for new event
3. Builds VCALENDAR with VEVENT (DTSTART, DTEND, SUMMARY, etc.)
4. HTTP PUT to `{calendar_url}/{uuid}.ics`
5. Server responds 201 Created
6. Returns confirmation with event UID

### Agent start agenda
1. `agent_start` event fires
2. Plugin queries CalDAV for events in next 8 hours
3. Formats compact agenda summary
4. Returns `{ "handled": true, "context": "Today's calendar: 10:00 Standup, 14:00-16:00 Deep work, 16:30 1:1 with Alex" }`
5. Host injects context string into agent session

### Status bar update
1. `turn_start` event fires
2. Plugin checks cached event list (or re-queries if >5 min stale)
3. Finds next upcoming event, calculates minutes until start
4. Returns UI action: `{ "action": "set_status", "text": "Next: Standup in 23m", "color": "cyan" }`
5. TUI renders in status bar

## Error Handling

| Error | Behavior |
|-------|----------|
| CalDAV server unreachable | Tool returns error string, agent can report to user |
| Invalid credentials | Tool returns auth error with setup instructions |
| No CALDAV_URL configured | `describe` output notes unconfigured state, tools return helpful error |
| Malformed iCalendar from server | Skip unparseable events, return parsed ones with warning |
| Timezone conversion failure | Fall back to UTC, note in output |
| Event not found (update/delete) | Return not-found error with UID |
| Calendar is read-only | `create_event` returns permission error |
