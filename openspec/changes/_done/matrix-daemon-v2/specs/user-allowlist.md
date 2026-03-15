# User Allowlist

## Purpose

Control which Matrix users can interact with the daemon.  Without this,
anyone in a joined room can prompt the agent and burn API credits / execute
tools.

## Requirements

### Allowlist configuration

The daemon MUST support a list of allowed Matrix user IDs, configured via:
1. `DaemonConfig.matrix_allowed_users: Vec<String>`
2. Environment variable `CLANKERS_MATRIX_ALLOWED_USERS` (comma-separated)
3. Config file at `~/.clankers/matrix.json` field `allowed_users`

GIVEN an allowlist is configured with `["@alice:example.com"]`
WHEN `@bob:example.com` sends a message
THEN the message is silently ignored

GIVEN an allowlist is configured with `["@alice:example.com"]`
WHEN `@alice:example.com` sends a message
THEN the message is processed normally

### Empty allowlist means allow-all

The daemon SHOULD treat an empty allowlist as "allow all users" for
backwards compatibility and single-user setups.

GIVEN no allowlist is configured (empty list)
WHEN any user sends a message
THEN the message is processed normally

### Denied message logging

The daemon MUST log denied messages at `info` level with the sender's
user ID, but MUST NOT send any response to the room (silent reject).
