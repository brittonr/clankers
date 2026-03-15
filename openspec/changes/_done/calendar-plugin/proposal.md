# calendar-plugin — CalDAV Calendar Integration

## Intent

Clankers has no concept of time beyond `chrono::Utc::now()`. It can't check
your calendar before scheduling a meeting, doesn't know you have a standup in
10 minutes, and can't create events when you say "block off tomorrow afternoon
for the refactor." Every other productivity tool integrates with calendars —
the coding agent should too.

This change adds a WASM plugin that speaks CalDAV to any standards-compliant
calendar server (Fastmail, Google via CalDAV, Radicale, Nextcloud, iCloud).
The agent gains tools to:

- List upcoming events (today, this week, arbitrary date range)
- Create events with title, time, duration, location, and attendees
- Update or cancel existing events
- Check free/busy availability for a time range
- Surface today's agenda at session start via the `agent_start` event

## Scope

### In Scope

- CalDAV client via HTTP (RFC 4791, RFC 6638)
- iCalendar parsing and generation (RFC 5545)
- `list_events` tool — query events by date range
- `create_event` tool — create a new calendar event
- `update_event` tool — modify an existing event (reschedule, rename, etc.)
- `delete_event` tool — cancel/remove an event
- `check_availability` tool — free/busy query for a time range
- Timezone-aware date/time handling (user-configured default timezone)
- `agent_start` event handler — inject today's agenda into session context
- TUI status bar segment showing next upcoming event
- Plugin UI widget showing today's agenda in a sidebar panel
- Multi-calendar support (discover all calendars on the account)
- Auth via token (Fastmail API token, Google app password, etc.)

### Out of Scope

- OAuth2 flows (use pre-obtained tokens, same pattern as clankers-email)
- Calendar subscriptions / webhooks (polling only, no push notifications)
- Recurring event rule editing (RRULE generation — can read, won't compose)
- Attendee RSVP management (can add attendees, won't track responses)
- CalDAV server implementation (client only)
- Calendar sharing / delegation
- Reminders / alarms that fire inside the TUI (display only, no timers)
- Integration with OS calendar apps
- Meeting room / resource booking

## Approach

Build a WASM plugin (`clankers-calendar`) following the same pattern as
`clankers-email`: Extism WASM module with `net` permission, CalDAV server
URL and credentials via `config_env`, and tool definitions in `plugin.json`.

The plugin uses CalDAV (HTTP + WebDAV + iCalendar) to communicate with the
server. CalDAV is the universal calendar protocol — Fastmail, Google, Apple,
Nextcloud, Radicale, and most self-hosted solutions support it. No
vendor-specific API is needed.

The agent can use these tools naturally in conversation:
- "What's on my calendar today?" → `list_events`
- "Block off 2-4pm tomorrow for deep work" → `create_event`
- "Am I free Friday afternoon?" → `check_availability`
- "Move the 1:1 to Thursday" → `update_event`

At `agent_start`, the plugin queries the next 8 hours of events and returns
a summary. This gets injected into the agent's context so it can be
time-aware ("You have a meeting in 30 minutes, should I wrap up this
refactor?").
