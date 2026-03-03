# matrix-daemon-v2 — Tasks

## Phase 1: Quick wins (no new deps)

- [ ] Typing indicators — spawn refresh task in `run_matrix_bridge`, cancel on response
- [ ] User allowlist — add `allowed_users` to `MatrixConfig`, check in bridge event loop
- [ ] Bot commands — dispatch `!restart`, `!status`, `!skills`, `!compact`, `!model`, `!help`
- [ ] Empty response re-prompt — check collected text in `run_matrix_prompt`, retry once
- [ ] Idle session reaping — background task in daemon, check `last_active` every 60s
- [ ] Stop ignoring slash commands — only skip `/` messages, not `!` messages (already done by above)

## Phase 2: Files (needs matrix-sdk media API)

- [ ] File receiving — handle `m.image`/`m.file`/`m.audio`/`m.video` events in the Matrix client
- [ ] Download attachments to `<session-dir>/attachments/`
- [ ] Prompt agent with file path (and base64 image block for vision models)
- [ ] File sending — scan response for `<sendfile>` tags, upload via `Room::send_attachment()`
- [ ] Path validation against sandbox policy

## Phase 3: Formatted responses (needs pulldown-cmark or comrak)

- [ ] Add markdown→HTML conversion
- [ ] Switch `send_text` to `text_html(plain, html)` in daemon Matrix responses
- [ ] Long response chunking at paragraph/code-block boundaries

## Phase 4: Proactive agent

- [ ] Heartbeat scheduler — background task, reads HEARTBEAT.md, prompts agent
- [ ] HEARTBEAT_OK suppression
- [ ] Heartbeat system prompt additions
- [ ] Trigger pipe — FIFO creation, reader task, prompt delivery
- [ ] Trigger pipe cleanup on session reap
- [ ] DaemonConfig fields: `heartbeat_interval`, `heartbeat_prompt`, `trigger_prompt`
