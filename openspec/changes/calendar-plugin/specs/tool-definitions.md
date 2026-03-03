# Calendar Tools — LLM Tool Definitions

## Purpose

The calendar plugin exposes five tools to the LLM agent via the standard
clankers plugin tool interface. Each tool is defined in `plugin.json` and
handled by the `handle_tool_call` WASM export function. All tools MUST
return JSON responses with `status`, `tool`, and `result` fields matching
the existing plugin convention.

## list_events

List calendar events within a date/time range.

### Input Schema

```json
{
  "type": "object",
  "properties": {
    "start": {
      "type": "string",
      "description": "Start of range (ISO 8601 date or datetime, e.g. '2026-03-03' or '2026-03-03T09:00'). Defaults to now."
    },
    "end": {
      "type": "string",
      "description": "End of range (ISO 8601 date or datetime). Defaults to end of start's day."
    },
    "calendar": {
      "type": "string",
      "description": "Calendar name to query. Defaults to all calendars."
    }
  }
}
```

### Behavior

The tool MUST query the CalDAV server for events overlapping the specified
range. Results MUST be sorted chronologically by start time.

GIVEN no parameters are provided
WHEN the agent calls list_events
THEN the tool returns events for today (midnight to midnight in configured timezone)

GIVEN start="2026-03-03" and end="2026-03-05"
WHEN the agent calls list_events
THEN the tool returns all events from March 3 00:00 to March 5 00:00

GIVEN a calendar name is specified
WHEN the agent calls list_events
THEN only events from that calendar are returned

GIVEN the date range contains no events
WHEN the agent calls list_events
THEN the tool returns `{ "status": "ok", "tool": "list_events", "result": "No events found for this period." }`

### Output Format

```json
{
  "status": "ok",
  "tool": "list_events",
  "result": "Found 3 events:\n\n• 10:00–10:30  Standup (Zoom)\n• 14:00–16:00  Deep work\n• 16:30–17:00  1:1 with Alex (Room 4B)",
  "events": [
    {
      "uid": "abc-123",
      "summary": "Standup",
      "start": "2026-03-03T10:00:00-05:00",
      "end": "2026-03-03T10:30:00-05:00",
      "location": "Zoom",
      "calendar": "Work"
    }
  ]
}
```

The `result` field MUST contain a human-readable formatted list. The `events`
array MUST contain structured data for programmatic use.

## create_event

Create a new calendar event.

### Input Schema

```json
{
  "type": "object",
  "properties": {
    "summary": {
      "type": "string",
      "description": "Event title/summary"
    },
    "start": {
      "type": "string",
      "description": "Start time (ISO 8601 datetime, e.g. '2026-03-04T14:00')"
    },
    "end": {
      "type": "string",
      "description": "End time (ISO 8601 datetime). Either end or duration is required."
    },
    "duration": {
      "type": "string",
      "description": "Duration (e.g. '1h', '30m', '1h30m'). Alternative to end."
    },
    "location": {
      "type": "string",
      "description": "Event location"
    },
    "description": {
      "type": "string",
      "description": "Event description/notes"
    },
    "attendees": {
      "type": "string",
      "description": "Comma-separated attendee email addresses"
    },
    "calendar": {
      "type": "string",
      "description": "Calendar to create event in. Defaults to configured default calendar."
    },
    "all_day": {
      "type": "boolean",
      "description": "If true, creates an all-day event. Only start date is used."
    }
  },
  "required": ["summary", "start"]
}
```

### Behavior

GIVEN summary and start are provided with duration="2h"
WHEN the agent calls create_event
THEN the tool creates a 2-hour event and returns the UID

GIVEN summary and start are provided with no end or duration
WHEN the agent calls create_event
THEN the tool creates a 1-hour event by default

GIVEN all_day=true and start="2026-03-04"
WHEN the agent calls create_event
THEN the tool creates an all-day event with VALUE=DATE

GIVEN attendees="alice@example.com,bob@example.com"
WHEN the agent calls create_event
THEN the tool includes ATTENDEE properties for each email

### Output Format

```json
{
  "status": "ok",
  "tool": "create_event",
  "result": "Created event 'Deep work' on Mar 4, 2026 14:00–16:00 (America/New_York)",
  "uid": "550e8400-e29b-41d4-a716-446655440000"
}
```

## update_event

Modify an existing calendar event.

### Input Schema

```json
{
  "type": "object",
  "properties": {
    "uid": {
      "type": "string",
      "description": "Event UID to update. Use list_events to find UIDs."
    },
    "summary": {
      "type": "string",
      "description": "New event title (omit to keep current)"
    },
    "start": {
      "type": "string",
      "description": "New start time (omit to keep current)"
    },
    "end": {
      "type": "string",
      "description": "New end time (omit to keep current)"
    },
    "duration": {
      "type": "string",
      "description": "New duration (omit to keep current)"
    },
    "location": {
      "type": "string",
      "description": "New location (omit to keep current)"
    },
    "description": {
      "type": "string",
      "description": "New description (omit to keep current)"
    }
  },
  "required": ["uid"]
}
```

### Behavior

The tool MUST fetch the current event, apply only the provided changes,
and PUT the modified event back with `If-Match` for ETag-based conflict
detection.

GIVEN a valid UID and new summary
WHEN the agent calls update_event
THEN only the summary is changed, all other fields are preserved

GIVEN an invalid or non-existent UID
WHEN the agent calls update_event
THEN the tool returns `{ "status": "error", "result": "Event not found: {uid}" }`

GIVEN a concurrent modification (ETag mismatch)
WHEN the agent calls update_event
THEN the tool returns a conflict error and suggests re-fetching

## delete_event

Remove a calendar event.

### Input Schema

```json
{
  "type": "object",
  "properties": {
    "uid": {
      "type": "string",
      "description": "Event UID to delete. Use list_events to find UIDs."
    }
  },
  "required": ["uid"]
}
```

### Behavior

GIVEN a valid UID
WHEN the agent calls delete_event
THEN the tool sends HTTP DELETE and confirms removal

GIVEN a non-existent UID
WHEN the agent calls delete_event
THEN the tool returns a not-found error

## check_availability

Check free/busy status for a time range.

### Input Schema

```json
{
  "type": "object",
  "properties": {
    "start": {
      "type": "string",
      "description": "Start of range to check (ISO 8601)"
    },
    "end": {
      "type": "string",
      "description": "End of range to check (ISO 8601)"
    }
  },
  "required": ["start", "end"]
}
```

### Behavior

The tool MUST query events in the range and compute free/busy slots.

GIVEN a time range with no events
WHEN the agent calls check_availability
THEN the tool returns `{ "status": "ok", "result": "You are free from 14:00 to 18:00", "available": true }`

GIVEN a time range that overlaps existing events
WHEN the agent calls check_availability
THEN the tool returns the conflicts and available gaps

### Output Format

```json
{
  "status": "ok",
  "tool": "check_availability",
  "result": "Partially busy:\n• 14:00–15:00  BUSY (Code review)\n• 15:00–16:00  FREE\n• 16:00–16:30  BUSY (Standup)\n• 16:30–18:00  FREE",
  "available": false,
  "busy_periods": [
    { "start": "14:00", "end": "15:00", "event": "Code review" },
    { "start": "16:00", "end": "16:30", "event": "Standup" }
  ],
  "free_periods": [
    { "start": "15:00", "end": "16:00" },
    { "start": "16:30", "end": "18:00" }
  ]
}
```
