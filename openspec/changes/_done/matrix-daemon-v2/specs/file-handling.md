# File Handling

## Purpose

Enable bidirectional file transfer over Matrix.  Users can send files to
the agent (images for analysis, documents to process), and the agent can
send files back (screenshots, generated images, diffs).

## Requirements

### Receiving files from Matrix

#### Download attachments

The daemon MUST handle Matrix media messages (`m.image`, `m.file`,
`m.audio`, `m.video`) by downloading the content to a session-local
directory.

GIVEN a user sends an image in the Matrix room
WHEN the daemon receives the `m.image` event
THEN the image is downloaded to `<session-dir>/attachments/<filename>`
AND the agent is prompted with: "User sent a file: <path> (<media_type>)"

#### Supported media types

The daemon MUST handle: `m.image`, `m.file`, `m.audio`, `m.video`.
Unknown media types SHOULD be downloaded as generic files.

#### Image content blocks

For `m.image` messages, the daemon SHOULD pass the image as a content
block (base64 image) to the agent if the model supports vision, in
addition to saving the file to disk.

### Sending files to Matrix

#### Sendfile tag protocol

The daemon MUST scan agent responses for `<sendfile>/path</sendfile>`
tags.  For each tag:

1. Strip the tag from the response text
2. Upload the file to the Matrix media repository
3. Send the file as an `m.file` (or `m.image` for image types) message

GIVEN the agent response contains `<sendfile>/tmp/screenshot.png</sendfile>`
WHEN the daemon processes the response
THEN `/tmp/screenshot.png` is uploaded to Matrix
AND sent as an `m.image` message in the room
AND the `<sendfile>` tag is removed from the text response

#### Multiple files

The daemon MUST support multiple `<sendfile>` tags in a single response.

#### Upload failure

If a file upload fails, the daemon MUST append an error note to the text
response: "(failed to send file <basename>: <error>)" and continue
processing remaining files.

#### Path validation

The daemon MUST only send files that exist and are readable.  The sandbox
path policy SHOULD be consulted to prevent exfiltrating sensitive files.

### Attachment directory lifecycle

The daemon SHOULD create `<session-dir>/attachments/` on first use and
clean it up when the session is reaped (idle timeout).
