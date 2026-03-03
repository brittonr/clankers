# Event Handling — Agent Lifecycle Integration

## Purpose

The calendar plugin subscribes to agent lifecycle events to provide ambient
calendar awareness. The `agent_start` event injects today's agenda into the
session context. The `turn_start` event refreshes the TUI status bar with
the next upcoming event.

## agent_start Handler

### Behavior

WHEN the `agent_start` event fires
THEN the plugin MUST query the CalDAV server for events in the next 8 hours
AND format a compact agenda summary
AND return it in the `context` field of the response

GIVEN the CalDAV server is reachable and has events
WHEN agent_start fires
THEN the response MUST include:
```json
{
  "event": "agent_start",
  "handled": true,
  "context": "📅 Today's calendar (America/New_York):\n  10:00  Standup (30m)\n  14:00  Deep work (2h)\n  16:30  1:1 with Alex (30m, Room 4B)\n\nNext event: Standup in 23 minutes.",
  "ui": [
    {
      "action": "set_status",
      "text": "📅 Standup in 23m",
      "color": "cyan"
    },
    {
      "action": "set_widget",
      "widget": {
        "type": "Box",
        "direction": "Vertical",
        "children": [
          { "type": "Text", "content": "📅 Today", "bold": true, "color": "cyan" },
          { "type": "List", "items": ["10:00 Standup", "14:00 Deep work", "16:30 1:1"], "selected": 0 }
        ]
      }
    }
  ]
}
```

GIVEN the CalDAV server is reachable but no events exist today
WHEN agent_start fires
THEN the response MUST include:
```json
{
  "event": "agent_start",
  "handled": true,
  "context": "📅 No upcoming events today. Calendar is clear."
}
```

GIVEN the CalDAV server is unreachable or credentials are invalid
WHEN agent_start fires
THEN the response MUST include `"handled": true` with a warning
AND the plugin MUST NOT cause the agent session to fail
```json
{
  "event": "agent_start",
  "handled": true,
  "context": "📅 Calendar unavailable (connection failed). Calendar tools may not work this session."
}
```

GIVEN no CalDAV credentials are configured
WHEN agent_start fires
THEN the response MUST be a no-op
```json
{
  "event": "agent_start",
  "handled": false,
  "message": "Calendar not configured. Set CALDAV_URL, CALDAV_USERNAME, CALDAV_PASSWORD."
}
```

### Context Injection

The `context` field returned from `agent_start` SHOULD be injected into
the agent's system prompt context so the LLM has calendar awareness
throughout the session. The host clankers plugin bridge already supports
this via the `context` response field.

## turn_start Handler

### Behavior

WHEN the `turn_start` event fires
THEN the plugin SHOULD check the cached event list
AND update the status bar with the next upcoming event

The plugin MUST NOT make a CalDAV request on every turn — it SHOULD use
cached data and only re-query if the cache is older than 5 minutes.

GIVEN events are cached and the next event is in less than 60 minutes
WHEN turn_start fires
THEN the status bar MUST show time remaining:
```json
{
  "event": "turn_start",
  "handled": true,
  "ui": {
    "action": "set_status",
    "text": "📅 Standup in 12m",
    "color": "yellow"
  }
}
```

GIVEN events are cached and the next event is more than 60 minutes away
WHEN turn_start fires
THEN the status bar SHOULD show the event time:
```json
{
  "event": "turn_start",
  "handled": true,
  "ui": {
    "action": "set_status",
    "text": "📅 Next: Deep work at 14:00",
    "color": "cyan"
  }
}
```

GIVEN no upcoming events remain today
WHEN turn_start fires
THEN the status bar SHOULD show calendar clear:
```json
{
  "event": "turn_start",
  "handled": true,
  "ui": {
    "action": "set_status",
    "text": "📅 Clear",
    "color": "green"
  }
}
```

### Color Coding

The status bar color MUST indicate urgency:

| Time Until Next Event | Color |
|----------------------|-------|
| ≤ 15 minutes | `red` |
| 16–60 minutes | `yellow` |
| > 60 minutes | `cyan` |
| No events | `green` |

### Event Cache

The plugin SHOULD maintain an in-memory cache of today's events:

- **Populated at:** `agent_start`
- **Refreshed when:** Cache age > 5 minutes AND a `turn_start` fires
- **Invalidated by:** `create_event`, `update_event`, `delete_event` tool calls
- **Scope:** Events from now until end of day in configured timezone

The cache is WASM module-scoped memory — it persists across function calls
within the same plugin instance but is lost on reload.
