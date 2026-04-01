## ADDED Requirements

### Requirement: Read email by ID
The `read_email` tool SHALL accept a required `id` parameter and return the full message content for that email.

#### Scenario: Read existing message
- **WHEN** `read_email` is called with a valid message `id`
- **THEN** the tool returns the message with fields: `id`, `from`, `to`, `cc`, `subject`, `date`, `body`

#### Scenario: Message not found
- **WHEN** `read_email` is called with an `id` that does not exist
- **THEN** the tool returns an error indicating the message was not found

### Requirement: Body text extraction
The `read_email` tool SHALL return plain text body content. If no text/plain part exists, it SHALL strip HTML tags and return the text content.

#### Scenario: Plain text message
- **WHEN** `read_email` fetches a message with a text/plain body
- **THEN** the tool returns the plain text content in the `body` field

#### Scenario: HTML-only message
- **WHEN** `read_email` fetches a message with only an HTML body
- **THEN** the tool strips HTML tags and returns the text content in the `body` field
- **AND** the response includes `html_stripped: true`

### Requirement: Attachment metadata
The `read_email` tool SHALL include attachment metadata (filename, size, content type) but SHALL NOT return binary attachment content.

#### Scenario: Message with attachments
- **WHEN** `read_email` fetches a message with file attachments
- **THEN** the response includes an `attachments` list with `filename`, `size`, and `content_type` for each

#### Scenario: Message without attachments
- **WHEN** `read_email` fetches a message with no attachments
- **THEN** the `attachments` list is empty

### Requirement: Read requires JMAP token
The `read_email` tool SHALL return a config error if `FASTMAIL_API_TOKEN` is not set.

#### Scenario: Missing token
- **WHEN** `read_email` is called without a configured JMAP token
- **THEN** the tool returns an error indicating the missing config

### Requirement: Read has no recipient allowlist restriction
The `read_email` tool SHALL NOT enforce the `CLANKERS_EMAIL_ALLOWED_RECIPIENTS` allowlist. Read access is governed solely by the JMAP token's permissions.

#### Scenario: Read unrestricted by allowlist
- **WHEN** `read_email` is called for a message from a sender not in the allowlist
- **THEN** the tool returns the message content without restriction
