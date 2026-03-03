# Typing Indicators

## Purpose

Show the bot as "typing" in the Matrix room while the agent is processing a
prompt.  This is the single highest-impact UX improvement — without it the
room feels dead during long tool runs.

## Requirements

### Start typing on prompt receipt

The daemon MUST send a typing indicator to the Matrix room immediately when
a user message is received and before the agent begins processing.

GIVEN a user sends a message in a Matrix room
WHEN the daemon receives the message
THEN a typing indicator is sent to that room before the agent prompt starts

### Stop typing on response

The daemon MUST cancel the typing indicator after sending the response
(or on error).

GIVEN the agent has finished processing (success or error)
WHEN the response is sent to the Matrix room
THEN the typing indicator is cleared

### Typing refresh during long runs

The daemon SHOULD periodically refresh the typing indicator during long-running
prompts, since Matrix typing notifications expire after ~30 seconds.

GIVEN the agent is still processing after 20 seconds
WHEN the typing indicator would expire
THEN the daemon refreshes it

## Implementation Notes

`matrix-sdk` provides `Room::typing_notice(true/false)`.  Spawn a background
task that sends `typing_notice(true)` every 20s until cancelled.
