# Empty Response Handling

## Purpose

When the agent performs tool calls but produces no text output (e.g. edits
files silently), the daemon currently sends an empty message to the room.
Instead, re-prompt for a summary.

## Requirements

### Re-prompt on empty response

The daemon MUST re-prompt the agent when the collected response text is
empty after a successful prompt execution.

GIVEN the agent processes a prompt successfully
WHEN the collected text response is empty or whitespace-only
THEN the daemon sends a follow-up prompt:
  "You completed some actions but your response contained no text.
   Briefly summarize what you did."

### Single retry only

The daemon MUST NOT re-prompt more than once.  If the second attempt also
returns empty, send a fallback message.

GIVEN the re-prompt also returns empty text
WHEN the daemon would send the response
THEN it sends "(completed actions — no summary available)" instead

### Tool-only responses are not empty

A response that contains only tool calls with no final text block MUST
trigger the re-prompt.  A response that contains actual text (even just
"Done.") MUST NOT trigger it.
