# matrix-daemon-v2

## Intent

The clankers daemon can already receive messages over Matrix and send text
responses back.  But compared to OpenCrow (a thin Go bridge that wraps pi),
the experience is lacking: no typing indicators, no file handling, no
proactive wake-ups, no access control, and no bot commands.  This change
closes those gaps so that Matrix becomes a first-class way to talk to your
clanker — not an afterthought.

## Scope

### In Scope

- Typing indicators while the agent is working
- User allowlist for the Matrix transport
- Bot commands (`!restart`, `!status`, `!skills`, `!compact`, `!model`)
- Empty response re-prompting
- Idle session timeout / reaping
- File receiving (download Matrix attachments, pass paths to agent)
- File sending (agent outputs file paths, daemon uploads to Matrix)
- Heartbeat scheduler (periodic HEARTBEAT.md check, proactive agent)
- Trigger pipes (named FIFO for external process integration)
- HTML/markdown formatted responses (not plain text)

### Out of Scope

- E2EE (matrix-sdk supports it, but setup/verification is a separate concern)
- New messaging backends (Telegram, Discord, Nostr — future changes)
- Channel abstraction trait (not needed yet — Matrix is the only channel)
- WebSocket gateway / browser access
- Changes to the iroh transport
- Changes to the TUI interactive mode

## Approach

All features are independent and can be implemented/shipped in any order.
Each one touches the daemon's Matrix bridge loop (`run_matrix_bridge` in
`src/modes/daemon.rs`) and/or the `clankers-matrix` crate.  No architectural
changes to the agent, session store, or tool system are required.
