## 1. JMAP Helpers

- [x] 1.1 Add `html_to_text` function — strip HTML tags, decode common entities (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&#xNN;`), collapse whitespace
- [x] 1.2 Add `resolve_mailbox_name` function — take a mailbox name string, call `Mailbox/get`, return the mailbox ID (reuse existing `find_mailbox_id`)
- [x] 1.3 Add unit tests for `html_to_text` (basic tags, nested tags, entities, empty input)

## 2. search_email Tool

- [x] 2.1 Add `handle_search_email` function — parse filter params (`from`, `to`, `subject`, `query`, `mailbox`, `after`, `before`), build JMAP `FilterCondition`
- [x] 2.2 Implement JMAP back-reference query — chain `Email/query` + `Email/get` with `#R1` in a single request, requesting `id`, `from`, `to`, `subject`, `receivedAt`, `preview`
- [x] 2.3 Implement pagination — accept `limit` (default 20, cap 100) and `offset` (default 0), map to JMAP `position` and `limit`
- [x] 2.4 Format search results as structured text output — one block per message with id, from, to, subject, date, preview
- [x] 2.5 Register `search_email` in `handle_tool_call` dispatch and `describe` output
- [x] 2.6 Add unit tests for filter building and result formatting (mock JMAP response parsing)

## 3. read_email Tool

- [x] 3.1 Add `handle_read_email` function — accept `id` param, call `Email/get` requesting full headers + `bodyValues` with `fetchTextBodyValues: true` and `fetchHTMLBodyValues: true`
- [x] 3.2 Implement body extraction — prefer `textBody` value, fall back to `htmlBody` → `html_to_text`, set `html_stripped` flag
- [x] 3.3 Implement attachment metadata extraction — iterate `attachments` array, collect `name`, `size`, `type` for each
- [x] 3.4 Handle not-found case — check JMAP `notFound` array, return clear error if message ID is missing
- [x] 3.5 Register `read_email` in `handle_tool_call` dispatch and `describe` output
- [x] 3.6 Add unit tests for body extraction logic (plain text path, HTML fallback path, no body)

## 4. Plugin Manifest

- [x] 4.1 Update `plugin.json` — add `search_email` and `read_email` to `tools` array
- [x] 4.2 Add `search_email` tool definition to `tool_definitions` with input schema (all filter params optional, limit/offset optional)
- [x] 4.3 Add `read_email` tool definition to `tool_definitions` with input schema (`id` required)

## 5. Integration & Build

- [x] 5.1 Rebuild WASM binary (`cargo build --target wasm32-unknown-unknown --release`)
- [x] 5.2 Add plugin-level tests in `src/plugin/tests/email_plugin.rs` — verify new tools appear in describe output, test missing-config rejection for both tools
- [x] 5.3 Add live integration tests in `tests/email_integration.rs` — `search_email` with no filters, `search_email` with `from` filter, `read_email` on a known message (gated on env vars)
