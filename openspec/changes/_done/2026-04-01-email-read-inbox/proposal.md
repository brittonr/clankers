## Why

The email plugin can send but not read. An agent that can compose replies but can't see what it's replying to is half-useful. Reading the inbox turns email into a full tool — the agent can triage, summarize, search, and act on messages without the user copy-pasting content in.

## What Changes

- Add `search_email` tool — query the inbox by sender, subject, date range, folder, or free text. Returns a summary list (id, from, subject, date, snippet).
- Add `read_email` tool — fetch a single message by ID. Returns full headers + body (plain text preferred, HTML converted to text as fallback).
- Add `list_mailboxes` enhancement — already exists but not wired into search. Will be used to resolve folder names to IDs for filtered searches.
- Recipient allowlist does not apply to reads (it gates outbound only).

## Capabilities

### New Capabilities
- `email-search`: Query mailboxes via JMAP `Email/query` + `Email/get`. Filter by from, to, subject, date, mailbox, and free-text. Paginated results.
- `email-read`: Fetch a single email by ID via JMAP `Email/get`. Return structured output: from, to, cc, subject, date, body text, attachment metadata.

### Modified Capabilities
<!-- No existing specs to modify — the send path and allowlist are unchanged. -->

## Impact

- `plugins/clankers-email/src/lib.rs` — new tool handlers, JMAP query logic, body extraction.
- `plugins/clankers-email/plugin.json` — add `search_email` and `read_email` to tool definitions.
- WASM binary rebuild required.
- No changes to the plugin SDK, host, or other plugins.
- New JMAP capabilities used: `Email/query` (already covered by `urn:ietf:params:jmap:mail`).
