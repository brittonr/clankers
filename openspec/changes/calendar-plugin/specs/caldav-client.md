# CalDAV Client — Protocol Implementation

## Purpose

The CalDAV client handles all HTTP communication with the calendar server.
It MUST implement the minimal CalDAV subset needed for event CRUD and
free/busy queries. It speaks HTTP with WebDAV extensions (PROPFIND, REPORT,
PUT, DELETE) and uses Basic authentication.

## Configuration

The client MUST read its configuration from Extism config keys, mapped
from environment variables via `plugin.json` `config_env`:

| Config Key | Env Var | Required | Example |
|------------|---------|----------|---------|
| `caldav_url` | `CALDAV_URL` | MUST | `https://caldav.fastmail.com/dav/calendars/user/me@fastmail.com/` |
| `caldav_username` | `CALDAV_USERNAME` | MUST | `me@fastmail.com` |
| `caldav_password` | `CALDAV_PASSWORD` | MUST | `app-specific-password` |
| `default_timezone` | `CLANKERS_TIMEZONE` | SHOULD | `America/New_York` |
| `default_calendar` | `CALDAV_DEFAULT_CALENDAR` | MAY | `personal` |

## Calendar Discovery

### PROPFIND for calendar list

The client MUST discover available calendars via PROPFIND on the CalDAV
principal URL.

```http
PROPFIND /dav/calendars/user/me@fastmail.com/ HTTP/1.1
Depth: 1
Content-Type: application/xml

<?xml version="1.0" encoding="UTF-8"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:displayname/>
    <D:resourcetype/>
    <C:supported-calendar-component-set/>
  </D:prop>
</D:propfind>
```

The client MUST filter responses to only include resources with
`<C:calendar/>` resource type. It SHOULD cache the calendar list for the
lifetime of the plugin instance.

## Event Queries

### REPORT calendar-query

The client MUST use `calendar-query` REPORT to fetch events in a date range.

```http
REPORT /dav/calendars/user/me@fastmail.com/personal/ HTTP/1.1
Depth: 1
Content-Type: application/xml

<?xml version="1.0" encoding="UTF-8"?>
<C:calendar-query xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:getetag/>
    <C:calendar-data/>
  </D:prop>
  <C:filter>
    <C:comp-filter name="VCALENDAR">
      <C:comp-filter name="VEVENT">
        <C:time-range start="20260303T000000Z" end="20260304T000000Z"/>
      </C:comp-filter>
    </C:comp-filter>
  </C:filter>
</C:calendar-query>
```

GIVEN a valid date range
WHEN the client sends a calendar-query REPORT
THEN the server responds with multiget containing VCALENDAR data
AND the client parses each VCALENDAR into Event structs

GIVEN a date range with no events
WHEN the client sends a calendar-query REPORT
THEN the client returns an empty event list

GIVEN the server returns a mix of parseable and unparseable events
WHEN the client processes the response
THEN it MUST return all parseable events
AND it SHOULD include a warning count for skipped events

## Event Creation

The client MUST create events via HTTP PUT to a new resource URL.

```http
PUT /dav/calendars/user/me@fastmail.com/personal/{uuid}.ics HTTP/1.1
Content-Type: text/calendar; charset=utf-8
If-None-Match: *

BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//clankers//calendar-plugin//EN
BEGIN:VEVENT
UID:{uuid}
DTSTART;TZID=America/New_York:20260304T140000
DTEND;TZID=America/New_York:20260304T160000
SUMMARY:Deep work
LOCATION:Home office
END:VEVENT
END:VCALENDAR
```

GIVEN valid event parameters
WHEN the client PUTs a new VCALENDAR resource
THEN the server responds with 201 Created
AND the client returns the event UID and ETag

GIVEN the `If-None-Match: *` header is set
WHEN a resource with that UID already exists
THEN the server responds with 412 Precondition Failed
AND the client returns a conflict error

## Event Update

The client MUST update events via HTTP PUT with `If-Match` and the
current ETag.

GIVEN a valid event UID and updated fields
WHEN the client fetches the current event, modifies fields, and PUTs back
THEN the server responds with 204 No Content
AND the client returns the new ETag

## Event Deletion

The client MUST delete events via HTTP DELETE.

GIVEN a valid event UID
WHEN the client sends DELETE to the event resource URL
THEN the server responds with 204 No Content
AND the client confirms deletion

## Free/Busy Query

The client SHOULD support free/busy queries via the CalDAV scheduling
extensions (RFC 6638), falling back to a client-side approach if the server
doesn't support it.

**Fallback approach:** Query all events in the range via `calendar-query`,
then compute free/busy slots client-side by checking for overlaps.

## Authentication

The client MUST use HTTP Basic authentication with the configured username
and password. All requests MUST be sent over HTTPS.

GIVEN valid credentials
WHEN the client sends an authenticated request
THEN the server processes it normally

GIVEN invalid credentials
WHEN the client sends a request
THEN the server responds with 401 Unauthorized
AND the client returns an error with setup instructions

## Timeout

All HTTP requests MUST have a 30-second timeout (matching the Extism
manifest timeout for net-enabled plugins).
