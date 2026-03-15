# calendar-plugin — Tasks

## Phase 1: Project scaffolding and iCalendar parser

- [ ] Create `plugins/clankers-calendar/` directory structure
- [ ] Create `Cargo.toml` with `extism-pdk`, `serde`, `serde_json` dependencies
- [ ] Create `build.sh` for `wasm32-unknown-unknown` compilation
- [ ] Create `plugin.json` manifest with all 5 tool definitions and event subscriptions
- [ ] Implement iCalendar parser: content line unfolding (RFC 5545 §3.1)
- [ ] Implement iCalendar parser: VEVENT property extraction (UID, SUMMARY, DTSTART, DTEND, DURATION, LOCATION, DESCRIPTION, ATTENDEE, STATUS)
- [ ] Implement iCalendar parser: date/time parsing (UTC, TZID, VALUE=DATE, floating)
- [ ] Implement iCalendar parser: escaped character handling (`\\`, `\n`, `\,`, `\;`)
- [ ] Implement iCalendar generator: Event → VCALENDAR text with CRLF and line folding
- [ ] Implement iCalendar generator: duration string parsing (`"2h"` → `PT2H`)
- [ ] Unit tests: parser roundtrip (generate → parse → compare)
- [ ] Unit tests: parse real-world VCALENDAR samples (Fastmail, Google, Apple)
- [ ] Unit tests: edge cases (all-day events, multi-day events, missing optional fields)

## Phase 2: CalDAV client

- [ ] Implement HTTP client wrapper using `extism_pdk::HttpRequest` for HTTPS
- [ ] Implement Basic auth header generation from configured credentials
- [ ] Implement PROPFIND calendar discovery (parse multistatus XML response)
- [ ] Implement REPORT calendar-query with time-range filter
- [ ] Implement PUT for event creation (`If-None-Match: *`)
- [ ] Implement PUT for event update (`If-Match: {etag}`)
- [ ] Implement DELETE for event removal
- [ ] Implement free/busy computation (client-side overlap detection from event list)
- [ ] Implement XML response parser for WebDAV multistatus (minimal, VEVENT extraction)
- [ ] Implement error mapping: HTTP status codes → user-friendly error messages
- [ ] Implement config loading from Extism config keys with validation
- [ ] Unit tests: XML multistatus parsing with mock responses
- [ ] Unit tests: auth header generation
- [ ] Unit tests: config validation (missing required fields, invalid URL)

## Phase 3: Tool handlers

- [ ] Implement `handle_tool_call` dispatcher (route by `tool` field)
- [ ] Implement `list_events` handler: date range parsing, CalDAV query, format output
- [ ] Implement `create_event` handler: build Event struct, generate iCal, PUT to server
- [ ] Implement `update_event` handler: fetch current → merge changes → PUT with ETag
- [ ] Implement `delete_event` handler: resolve UID to href → DELETE
- [ ] Implement `check_availability` handler: query events → compute free/busy slots
- [ ] Implement default behaviors: no-arg list_events returns today, no-duration defaults to 1h
- [ ] Implement `describe` export: return plugin metadata with tool list
- [ ] Implement `handle_command` export for `/calendar` slash command
- [ ] Implement human-friendly output formatting (bullet list with times and locations)
- [ ] Unit tests: each tool with mock CalDAV responses
- [ ] Unit tests: error paths (server down, bad credentials, event not found)

## Phase 4: Event handlers and UI

- [ ] Implement `on_event` dispatcher (route by event type)
- [ ] Implement `agent_start` handler: query next 8h, format agenda, return context + UI
- [ ] Implement `turn_start` handler: check cache age, compute next event, return status bar UI
- [ ] Implement in-memory event cache with 5-minute TTL
- [ ] Implement cache invalidation on create/update/delete tool calls
- [ ] Implement status bar color coding (red ≤15m, yellow 16-60m, cyan >60m, green clear)
- [ ] Implement agenda widget (Box > Text + List) for sidebar panel
- [ ] Implement time-until-event calculation with human formatting ("in 23m", "at 14:00")
- [ ] Unit tests: agent_start with events, without events, server unreachable, unconfigured
- [ ] Unit tests: turn_start cache behavior (fresh vs stale)
- [ ] Unit tests: color coding thresholds

## Phase 5: Build, integration test, documentation

- [ ] Build WASM module, verify it loads in clankers PluginManager
- [ ] Verify plugin discovery finds clankers-calendar and reads manifest
- [ ] Verify `has_function` for all exports (handle_tool_call, on_event, describe)
- [ ] Verify `call_plugin` with mock tool call JSON parses correctly
- [ ] Integration test: list_events against a test CalDAV server (Radicale in CI)
- [ ] Integration test: create → list → update → list → delete → list lifecycle
- [ ] Integration test: agent_start event returns valid context string
- [ ] Verify WASM binary size is reasonable (< 2MB)
- [ ] Write README.md with setup instructions for Fastmail, Google, Nextcloud
- [ ] Document env var configuration in README
- [ ] Add clankers-calendar to `plugins/` gitignore for built WASM artifact
