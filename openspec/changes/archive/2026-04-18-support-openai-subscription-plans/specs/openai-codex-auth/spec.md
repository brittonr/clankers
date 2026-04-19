## ADDED Requirements

### Requirement: Provider-aware auth commands support OpenAI Codex subscriptions

The system SHALL let users authenticate a ChatGPT Plus or Pro subscription as provider `openai-codex` from both CLI and interactive slash commands. Auth state for `openai-codex` SHALL be stored separately from `anthropic` and API-key `openai` credentials.

#### Scenario: CLI login starts an OpenAI Codex flow

- GIVEN no `openai-codex` account is configured
- WHEN the user runs `clankers auth login --provider openai-codex`
- THEN clankers prints or opens an OpenAI Codex authorization URL for that provider
- AND stores provider-specific verifier and state needed to finish login without overwriting Anthropic login state

#### Scenario: Two OpenAI Codex logins for different account names do not collide

- GIVEN two `openai-codex` login flows are started for different clankers account names supplied through the existing `--account <name>` CLI flag or equivalent slash-command account argument
- WHEN each flow stores its pending verifier and state
- THEN the pending login state remains isolated by provider and exact account name
- AND completing one flow does not overwrite or invalidate the other pending flow

#### Scenario: CLI login completion persists provider-scoped credentials

- GIVEN login for `openai-codex` is in progress
- WHEN the user completes the OAuth callback and provides the code or callback URL
- THEN clankers stores OAuth credentials under provider `openai-codex`
- AND marks the requested account active for `openai-codex`
- AND leaves existing `anthropic` and `openai` credentials unchanged

#### Scenario: Slash-command login uses the same provider flow

- GIVEN interactive mode is running
- WHEN the user runs `/login openai-codex`
- THEN interactive mode starts or completes the same provider-specific login flow
- AND successful login triggers provider credential reload without restarting the session

#### Scenario: Omitted provider keeps the Anthropic default

- GIVEN the user does not specify a provider
- WHEN the user runs `clankers auth login` or `/login`
- THEN clankers starts the same Anthropic-compatible default flow it uses today
- AND does not silently switch the default login provider to `openai-codex`

### Requirement: OpenAI Codex OAuth uses OpenAI OAuth endpoints

The system SHALL start `openai-codex` OAuth at `https://auth.openai.com/oauth/authorize` and use `https://auth.openai.com/oauth/token` for both code exchange and token refresh.

#### Scenario: Login starts against the OpenAI authorize endpoint

- GIVEN the user starts an `openai-codex` login flow
- WHEN clankers generates the authorization URL
- THEN that URL targets `https://auth.openai.com/oauth/authorize`
- AND includes `response_type=code`, `client_id=app_EMoamEEZ73f0CkXaXp7hrann`, `redirect_uri=http://localhost:1455/auth/callback`, `scope=openid profile email offline_access`, `code_challenge`, `code_challenge_method=S256`, and `state`
- AND includes `id_token_add_organizations=true`, `codex_cli_simplified_flow=true`, and `originator=pi`

#### Scenario: Code exchange and refresh use the OpenAI token endpoint

- GIVEN clankers completes or refreshes an `openai-codex` credential
- WHEN it exchanges an authorization code or refresh token
- THEN it sends that request to `https://auth.openai.com/oauth/token`
- AND code exchange uses form fields `grant_type=authorization_code`, `client_id=app_EMoamEEZ73f0CkXaXp7hrann`, `code`, `code_verifier`, and `redirect_uri=http://localhost:1455/auth/callback`
- AND refresh uses form fields `grant_type=refresh_token`, `client_id=app_EMoamEEZ73f0CkXaXp7hrann`, and `refresh_token`

### Requirement: OpenAI Codex account identity is derived from the active OAuth token

The system SHALL derive the backend-required ChatGPT account identifier from the active `openai-codex` OAuth access token and make it available to request code without requiring a separate manual configuration value.

#### Scenario: Login derives the account identifier

- GIVEN an `openai-codex` login completes successfully
- WHEN clankers stores the returned credentials
- THEN it derives `chatgpt_account_id` from JWT payload path `payload["https://api.openai.com/auth"]["chatgpt_account_id"]`
- AND makes that value available to the active provider for request headers

#### Scenario: Refresh updates the derived account identifier

- GIVEN an `openai-codex` credential refresh returns a new access token
- WHEN the refreshed credential is loaded
- THEN clankers re-derives the account identifier from the refreshed token
- AND subsequent requests use the refreshed identifier source

#### Scenario: Missing or malformed claim blocks Codex use

- GIVEN an `openai-codex` access token does not contain a usable `chatgpt_account_id` inside the `https://api.openai.com/auth` claim payload
- WHEN login or refresh loads that credential
- THEN clankers surfaces a provider-auth error for `openai-codex`
- AND does not send Codex requests without a valid `chatgpt-account-id`

### Requirement: OpenAI Codex credentials refresh automatically

The system SHALL refresh `openai-codex` OAuth credentials before expiry and reuse refreshed tokens for subsequent requests.

#### Scenario: Proactive refresh updates the auth store

- GIVEN an `openai-codex` access token is near expiry and a refresh token is present
- WHEN background credential refresh runs
- THEN the system exchanges the refresh token for fresh credentials
- AND updates the persisted auth store entry for provider `openai-codex`

#### Scenario: Unauthorized response triggers one refresh-and-retry

- GIVEN an `openai-codex` request returns HTTP 401 and a refresh token is present
- WHEN the provider handles the failure
- THEN it performs one token refresh
- AND retries the request with the refreshed access token

### Requirement: Auth status and account management are provider-aware

The system SHALL show, switch, and remove `openai-codex` accounts independently from other providers.

#### Scenario: Status lists provider-specific accounts

- GIVEN accounts exist for `anthropic`, `openai`, and `openai-codex`
- WHEN the user runs `clankers auth status --all` or `/account --all`
- THEN output identifies accounts by provider
- AND `openai-codex` accounts show OAuth validity or expiry separately from Anthropic accounts

#### Scenario: Explicit provider-scoped status shows only OpenAI Codex accounts

- GIVEN accounts exist for multiple providers including `openai-codex`
- WHEN the user runs `clankers auth status --provider openai-codex` or the slash-command equivalent
- THEN output shows only `openai-codex` account state
- AND includes entitlement state such as entitled, authenticated-but-not-entitled, or entitlement-check-failed

#### Scenario: Omitted provider keeps Anthropic-compatible status output

- GIVEN the user does not request `--all` and does not specify a provider
- WHEN the user runs `clankers auth status` or `/account`
- THEN clankers shows the same Anthropic-compatible default summary it uses today
- AND does not silently switch the default status target to `openai-codex`

#### Scenario: Switching or logout affects only the selected provider

- GIVEN provider accounts exist for both `anthropic` and `openai-codex`
- WHEN the user switches or logs out an `openai-codex` account using an explicit provider selection
- THEN only `openai-codex` account state changes
- AND Anthropic account state stays unchanged

#### Scenario: Omitted provider keeps Anthropic-compatible switch and logout behavior

- GIVEN the user does not specify a provider for switch or logout
- WHEN the user runs the existing switch or logout flow
- THEN clankers targets Anthropic account state by default
- AND does not apply the operation to `openai-codex` accounts unless the provider is explicitly selected

### Requirement: Help text and provider docs describe Codex auth and limitations

The system SHALL keep CLI help, slash help, and provider docs aligned with `openai-codex` behavior so users can discover the provider without confusing it with API-key `openai`.

#### Scenario: Help text explains provider name, account naming, and model selection

- GIVEN the user reads auth help text or provider docs
- WHEN `openai-codex` support is documented
- THEN the docs describe `openai-codex` as separate from API-key `openai`
- AND explain that clankers reuses existing local account names through `--account <name>` or equivalent slash-account syntax
- AND show how users select `openai-codex` models distinctly from API-key `openai` models

#### Scenario: Docs describe plan limits and unsupported accounts

- GIVEN the user reads auth help text or provider docs
- WHEN `openai-codex` support is documented
- THEN the docs state that ChatGPT Plus or Pro personal subscriptions are required
- AND explain that unsupported or non-entitled accounts appear as authenticated but unavailable for Codex use
- AND explain that explicit or resumed `openai-codex` requests fail closed instead of falling back to API-key `openai`
