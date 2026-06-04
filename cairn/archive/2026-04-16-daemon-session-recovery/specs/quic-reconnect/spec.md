## ADDED Requirements

### Requirement: QUIC re-attach after disconnect
A remote client that loses its QUIC stream SHALL be able to re-attach to the same session by opening a new bidirectional stream with a `DaemonRequest::Attach` containing the session ID.

#### Scenario: Transient network failure
- **WHEN** the QUIC stream drops due to network interruption
- **AND** the client opens a new stream with the same session ID
- **THEN** the client SHALL receive an `AttachResponse::Ok` and resume receiving events

#### Scenario: Re-attach after daemon restart
- **WHEN** the daemon restarts while a remote client is connected
- **AND** the client reconnects to the iroh endpoint and sends `DaemonRequest::Attach`
- **THEN** the daemon SHALL recover the session (if suspended) and attach the client

### Requirement: Client-side QUIC reconnect retry
The remote attach TUI SHALL detect QUIC stream disconnection and attempt reconnection with exponential backoff, mirroring the existing Unix socket reconnect behavior.

#### Scenario: Automatic retry on stream loss
- **WHEN** the QUIC event stream closes unexpectedly
- **THEN** the client SHALL attempt to open a new QUIC bidirectional stream
- **AND** retry up to 5 times with exponential backoff (1s, 2s, 4s, 8s, 16s)

#### Scenario: Reconnect succeeds
- **WHEN** a retry attempt succeeds
- **THEN** the client SHALL display the session with full history replay
- **AND** the reconnection status message SHALL be cleared

#### Scenario: Reconnect after iroh endpoint reconnect
- **WHEN** the underlying iroh connection is re-established after a daemon restart
- **AND** the client retries `DaemonRequest::Attach` with the previous session ID
- **THEN** the session SHALL be recovered and the client attached

### Requirement: Session ID tracking for reconnect
The remote attach client SHALL persist the session ID from the initial `AttachResponse` so that reconnection targets the same session rather than creating a new one.

#### Scenario: Session ID preserved across reconnect
- **WHEN** a client successfully attaches and receives session ID "abc123"
- **AND** the stream disconnects
- **THEN** the reconnect attempt SHALL include session ID "abc123" in the handshake
