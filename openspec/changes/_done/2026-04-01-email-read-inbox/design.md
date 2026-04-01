## Context

The `clankers-email` plugin is an Extism WASM module that talks to Fastmail's JMAP API. It currently supports `send_email` and `list_mailboxes`. All JMAP plumbing — session discovery, auth, method calls — already exists. Reading email adds two new JMAP method pairs: `Email/query` for searching and `Email/get` for fetching message content.

The plugin runs in the WASM sandbox with `net` permission scoped to `api.fastmail.com`. Config is injected via Extism host config mapped from env vars (`FASTMAIL_API_TOKEN`, etc.).

## Goals / Non-Goals

**Goals:**
- Let the agent search email by sender, subject, date range, folder, and free text.
- Let the agent read a single email's full content by message ID.
- Return plain text bodies. Fall back to stripping HTML when no text part exists.
- Paginate search results with a sensible default limit (20) and offset support.

**Non-Goals:**
- Modifying emails (move, delete, mark read/unread) — future change.
- Attachments — return metadata (filename, size, type) but not binary content.
- Threads/conversations — return flat message lists, not grouped threads.
- Reply/forward composition — stays in `send_email`.
- Any provider other than Fastmail.

## Decisions

### 1. Two tools, not one

**Decision:** Separate `search_email` (returns list with snippets) and `read_email` (returns full message).

**Rationale:** Search results can be large. Returning full bodies for 20 messages wastes context window. The two-step pattern (search → pick → read) is standard and maps to how JMAP splits `Email/query` from `Email/get`.

**Alternative:** Single `get_email` tool with optional query params. Rejected — conflates list and detail views, makes pagination awkward.

### 2. JMAP `Email/query` + `Email/get` in a single round-trip

**Decision:** Use JMAP's back-reference syntax (`#R1`) to chain query → get in one HTTP call.

**Rationale:** Cuts latency in half vs two separate calls. JMAP supports this natively via `resultReference` in method calls.

### 3. Body extraction: text/plain preferred, HTML stripped as fallback

**Decision:** Request `bodyValues` with `fetchTextBodyValues: true`. If no plain text part, request HTML body and strip tags with a simple regex-based approach inside WASM.

**Rationale:** Full HTML parsing pulls in heavy dependencies. For agent consumption, a basic tag-strip (`<[^>]+>` → remove, decode `&amp;` etc.) is sufficient. Agents don't need perfect rendering.

**Alternative:** Return raw HTML and let the agent deal with it. Rejected — bloats token usage.

### 4. No read-side allowlist

**Decision:** The recipient allowlist (`CLANKERS_EMAIL_ALLOWED_RECIPIENTS`) only gates outbound. Reads are unrestricted — the agent can see any email the Fastmail token has access to.

**Rationale:** The token's scopes already control read access. Adding a separate read allowlist adds config burden with little security value — if the token can see it, the agent should too.

### 5. Search filter mapping

**Decision:** Map tool parameters directly to JMAP `FilterCondition` fields:

| Tool param | JMAP filter |
|---|---|
| `from` | `from` (substring match) |
| `to` | `to` (substring match) |
| `subject` | `subject` (substring match) |
| `query` | `text` (full-text search) |
| `mailbox` | Resolve name → ID via `Mailbox/get`, then `inMailbox` |
| `after` / `before` | `after` / `before` (ISO 8601 date) |

**Rationale:** Direct mapping keeps the tool params predictable. JMAP's built-in filtering handles the heavy lifting server-side.

### 6. Pagination defaults

**Decision:** `limit` defaults to 20, `offset` defaults to 0. Max limit capped at 100.

**Rationale:** 20 fits comfortably in an agent's context. Cap at 100 prevents accidental token-budget blowout.

## Risks / Trade-offs

- **HTML stripping is lossy** → Acceptable for agent use. Users needing rich rendering should use a mail client. Mitigation: return a `has_html` flag so the agent knows when content was simplified.
- **Large mailboxes, slow queries** → Fastmail's JMAP handles this server-side. Mitigation: default limit of 20, agent can narrow filters if needed.
- **Mailbox name resolution adds a round-trip** → Only when `mailbox` param is a name (not an ID). Mitigation: cache is not worth the complexity for a WASM plugin; the extra call is fast.
