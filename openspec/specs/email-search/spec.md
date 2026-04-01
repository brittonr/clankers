## ADDED Requirements

### Requirement: Search email by filters
The `search_email` tool SHALL accept optional filter parameters: `from`, `to`, `subject`, `query`, `mailbox`, `after`, `before`. All filters are AND-combined. If no filters are provided, the tool SHALL return the most recent messages.

#### Scenario: Search by sender
- **WHEN** `search_email` is called with `from: "alice@example.com"`
- **THEN** the tool returns messages where the `From` header contains `alice@example.com`

#### Scenario: Search by subject
- **WHEN** `search_email` is called with `subject: "invoice"`
- **THEN** the tool returns messages whose subject contains "invoice" (substring match)

#### Scenario: Full-text search
- **WHEN** `search_email` is called with `query: "quarterly report"`
- **THEN** the tool returns messages matching the free-text search across all fields

#### Scenario: Filter by mailbox name
- **WHEN** `search_email` is called with `mailbox: "Inbox"`
- **THEN** the tool resolves "Inbox" to a mailbox ID and returns messages from that mailbox only

#### Scenario: Filter by date range
- **WHEN** `search_email` is called with `after: "2025-01-01"` and `before: "2025-02-01"`
- **THEN** the tool returns messages received within that date range

#### Scenario: Combined filters
- **WHEN** `search_email` is called with `from: "bob@co.com"` and `subject: "deploy"`
- **THEN** the tool returns only messages matching both conditions

#### Scenario: No filters
- **WHEN** `search_email` is called with no filter parameters
- **THEN** the tool returns the most recent messages sorted by date descending

### Requirement: Search results are paginated
The `search_email` tool SHALL accept `limit` (default 20, max 100) and `offset` (default 0) parameters for pagination.

#### Scenario: Default pagination
- **WHEN** `search_email` is called with no pagination parameters
- **THEN** the tool returns at most 20 results starting from the most recent

#### Scenario: Custom limit and offset
- **WHEN** `search_email` is called with `limit: 5` and `offset: 10`
- **THEN** the tool returns at most 5 results starting from position 10

#### Scenario: Limit capped at 100
- **WHEN** `search_email` is called with `limit: 500`
- **THEN** the tool caps the limit to 100 and returns at most 100 results

### Requirement: Search results contain summary fields
Each result in the `search_email` response SHALL include: `id`, `from`, `to`, `subject`, `date`, and `preview` (short text snippet).

#### Scenario: Result format
- **WHEN** `search_email` returns results
- **THEN** each result contains `id`, `from`, `to`, `subject`, `date`, and `preview` fields

#### Scenario: Empty results
- **WHEN** `search_email` matches no messages
- **THEN** the tool returns an empty list with a message indicating no matches

### Requirement: Search requires JMAP token
The `search_email` tool SHALL return a config error if `FASTMAIL_API_TOKEN` is not set.

#### Scenario: Missing token
- **WHEN** `search_email` is called without a configured JMAP token
- **THEN** the tool returns an error indicating the missing config

### Requirement: Search uses single JMAP round-trip
The `search_email` tool SHALL chain `Email/query` and `Email/get` using JMAP back-references in a single HTTP request.

#### Scenario: Query and get in one call
- **WHEN** `search_email` executes a search
- **THEN** it sends one JMAP request containing both `Email/query` and `Email/get` with a `#R1` back-reference
