# Plugin Manifest — plugin.json

## Purpose

The `plugin.json` manifest declares the calendar plugin's metadata, tools,
events, permissions, and configuration. It MUST follow the `PluginManifest`
schema defined in `src/plugin/manifest.rs`.

## Manifest

```json
{
  "name": "clankers-calendar",
  "version": "0.1.0",
  "description": "CalDAV calendar integration — list, create, update, and delete events on any CalDAV server",
  "wasm": "clankers_calendar.wasm",
  "kind": "extism",
  "permissions": ["net", "ui"],
  "allowed_hosts": [],
  "config_env": {
    "caldav_url": "CALDAV_URL",
    "caldav_username": "CALDAV_USERNAME",
    "caldav_password": "CALDAV_PASSWORD",
    "default_timezone": "CLANKERS_TIMEZONE",
    "default_calendar": "CALDAV_DEFAULT_CALENDAR"
  },
  "tools": ["list_events", "create_event", "update_event", "delete_event", "check_availability"],
  "commands": ["calendar"],
  "events": ["agent_start", "turn_start"],
  "tool_definitions": [
    {
      "name": "list_events",
      "description": "List calendar events in a date/time range from your CalDAV calendar. Returns events sorted chronologically with title, time, location. Defaults to today if no range specified.",
      "handler": "handle_tool_call",
      "input_schema": {
        "type": "object",
        "properties": {
          "start": {
            "type": "string",
            "description": "Start of range (ISO 8601 date or datetime). Defaults to now."
          },
          "end": {
            "type": "string",
            "description": "End of range (ISO 8601). Defaults to end of day."
          },
          "calendar": {
            "type": "string",
            "description": "Calendar name to query. Defaults to all."
          }
        }
      }
    },
    {
      "name": "create_event",
      "description": "Create a new calendar event on your CalDAV calendar. Supports title, start/end time, duration, location, description, and attendees. Returns the event UID.",
      "handler": "handle_tool_call",
      "input_schema": {
        "type": "object",
        "properties": {
          "summary": {
            "type": "string",
            "description": "Event title"
          },
          "start": {
            "type": "string",
            "description": "Start time (ISO 8601 datetime)"
          },
          "end": {
            "type": "string",
            "description": "End time (ISO 8601). Either end or duration required."
          },
          "duration": {
            "type": "string",
            "description": "Duration (e.g. '1h', '30m', '1h30m')"
          },
          "location": {
            "type": "string",
            "description": "Event location"
          },
          "description": {
            "type": "string",
            "description": "Event notes"
          },
          "attendees": {
            "type": "string",
            "description": "Comma-separated attendee emails"
          },
          "calendar": {
            "type": "string",
            "description": "Target calendar name"
          },
          "all_day": {
            "type": "boolean",
            "description": "Create all-day event"
          }
        },
        "required": ["summary", "start"]
      }
    },
    {
      "name": "update_event",
      "description": "Update an existing calendar event. Requires the event UID (from list_events). Only provided fields are changed; others are preserved.",
      "handler": "handle_tool_call",
      "input_schema": {
        "type": "object",
        "properties": {
          "uid": {
            "type": "string",
            "description": "Event UID to update"
          },
          "summary": { "type": "string", "description": "New title" },
          "start": { "type": "string", "description": "New start time" },
          "end": { "type": "string", "description": "New end time" },
          "duration": { "type": "string", "description": "New duration" },
          "location": { "type": "string", "description": "New location" },
          "description": { "type": "string", "description": "New notes" }
        },
        "required": ["uid"]
      }
    },
    {
      "name": "delete_event",
      "description": "Delete a calendar event by UID. Use list_events first to find the UID of the event to delete.",
      "handler": "handle_tool_call",
      "input_schema": {
        "type": "object",
        "properties": {
          "uid": {
            "type": "string",
            "description": "Event UID to delete"
          }
        },
        "required": ["uid"]
      }
    },
    {
      "name": "check_availability",
      "description": "Check if you are free or busy during a time range. Shows conflicting events and available gaps. Useful before scheduling.",
      "handler": "handle_tool_call",
      "input_schema": {
        "type": "object",
        "properties": {
          "start": {
            "type": "string",
            "description": "Start of range (ISO 8601)"
          },
          "end": {
            "type": "string",
            "description": "End of range (ISO 8601)"
          }
        },
        "required": ["start", "end"]
      }
    }
  ]
}
```

## Permissions

- **`net`**: REQUIRED. CalDAV communication over HTTPS.
- **`ui`**: REQUIRED. Status bar segment and agenda widget.

## allowed_hosts

The `allowed_hosts` array SHOULD be left empty in the distributed manifest
(allowing all hosts) because CalDAV server URLs vary per user. Users who
want to restrict network access can override this in a local config.

Alternatively, the plugin MAY parse the hostname from `CALDAV_URL` at
runtime and only connect to that host. This is enforced by the CalDAV
client implementation, not by the Extism manifest.

## Slash Command

The `calendar` command allows direct interaction without going through
the LLM:

- `/calendar` — show today's agenda
- `/calendar tomorrow` — show tomorrow's events
- `/calendar week` — show this week's events

This is registered via the `commands` field and handled by the
`handle_command` WASM export.

## File Layout

```
plugins/clankers-calendar/
├── Cargo.toml
├── build.sh
├── plugin.json
├── src/
│   ├── lib.rs          # Extism exports: handle_tool_call, on_event, describe
│   ├── caldav.rs       # CalDAV HTTP client
│   ├── icalendar.rs    # iCalendar parser/generator
│   ├── tools.rs        # Tool dispatch and formatting
│   └── cache.rs        # In-memory event cache
└── clankers_calendar.wasm  # Built artifact (gitignored)
```
