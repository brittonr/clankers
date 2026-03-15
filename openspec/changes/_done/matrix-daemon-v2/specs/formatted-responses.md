# Formatted Responses

## Purpose

Send agent responses as HTML-formatted Matrix messages instead of plain text.
Matrix clients render HTML natively — code blocks get syntax highlighting,
headers get structure, links become clickable.

## Requirements

### HTML message format

The daemon MUST send responses using `RoomMessageEventContent::text_html(plain, html)`
where `plain` is the raw markdown text and `html` is the rendered HTML.

GIVEN the agent responds with markdown containing code blocks and headers
WHEN the daemon sends the response to the Matrix room
THEN the message has both a plain-text body (fallback) and an HTML formatted_body

### Markdown to HTML conversion

The daemon MUST convert the agent's markdown response to HTML.  Use
`pulldown-cmark` (already common in the Rust ecosystem) or `comrak`.

### Long response chunking

Matrix has a practical message size limit (~65KB body, but most clients
struggle with messages over ~4000 characters).  The daemon SHOULD split
long responses into multiple messages at natural boundaries (paragraph
breaks, code block boundaries).

GIVEN the agent response is 8000 characters
WHEN the daemon sends the response
THEN it is split into 2-3 messages at paragraph boundaries
AND each chunk is independently formatted as HTML

### Code block preservation

Code blocks MUST be preserved intact — never split a fenced code block
across messages.
