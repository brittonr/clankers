# router-oauth-refresh Specification

## Purpose
TBD - created by archiving change improve-model-connection-errors. Update Purpose after archive.
## Requirements
### Requirement: Router proxy refreshes OAuth tokens proactively
The clanker-router binary SHALL run a background task that refreshes OAuth tokens from its own auth store (`~/.config/clanker-router/auth.json`) before they expire, using the same 5-minute-before-expiry strategy as `clankers-provider::CredentialManager`.

#### Scenario: Token refreshed before expiry
- **WHEN** the router proxy is running with an OAuth credential that expires in 10 minutes
- **THEN** the background task refreshes the token before it expires and updates the in-memory credential

#### Scenario: Proxy stays operational across token boundary
- **WHEN** a request arrives after the original token's expiry time
- **AND** the proactive refresh succeeded earlier
- **THEN** the request uses the refreshed token and succeeds

### Requirement: Router proxy refreshes reactively on 401
When the Anthropic backend receives HTTP 401, the router binary SHALL attempt an inline token refresh using the stored refresh token before retrying or propagating the error.

#### Scenario: Expired token recovered by reactive refresh
- **WHEN** the proactive refresh failed (e.g., transient network error) and the token expired
- **AND** a request triggers a 401 from Anthropic
- **THEN** the router refreshes the token inline, retries the request with the new token, and succeeds

### Requirement: Refreshed tokens persisted with file locking
After a successful token refresh, the router SHALL write the new credentials to its auth store using exclusive file locking to prevent corruption from concurrent instances.

#### Scenario: Two router instances refresh concurrently
- **WHEN** two router processes both attempt to refresh and save at the same time
- **THEN** the auth store file is not corrupted and contains a valid credential

### Requirement: Refresh failure does not crash the proxy
If the OAuth refresh endpoint is unreachable or returns an error, the proxy SHALL log a warning and continue operating with the existing (possibly expired) token. The next incoming request will trigger a reactive refresh attempt.

#### Scenario: Refresh endpoint unreachable
- **WHEN** the proactive refresh task cannot reach the OAuth endpoint
- **THEN** a warning is logged and the proxy continues running
- **AND** the next request that gets a 401 triggers another refresh attempt

### Requirement: Refresh loop exits on proxy shutdown
The background refresh task SHALL use a weak reference or cancellation token so it exits when the proxy shuts down, without leaking the task.

#### Scenario: Proxy shutdown during refresh sleep
- **WHEN** the proxy is shut down while the refresh task is sleeping until the next refresh window
- **THEN** the refresh task exits without blocking shutdown

