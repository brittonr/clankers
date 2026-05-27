# provider-auth Specification

## Requirements

### Requirement: Provider compatibility remains explicit
Provider compatibility behavior MUST be encoded as reviewable scenarios before implementation closes.

#### Scenario: omitted provider keeps Anthropic defaults
- GIVEN a CLI or slash auth command is invoked with an omitted provider
- WHEN provider resolution runs
- THEN the command MUST use the Anthropic provider default rather than selecting openai-codex

#### Scenario: malformed chatgpt claim is rejected
- GIVEN the OpenAI access token has a missing or malformed claim for chatgpt_account_id
- WHEN provider credential loading or refresh derives the account identity
- THEN the operation MUST surface an auth error before any chatgpt-account-id value is used

#### Scenario: provider-scoped status is explicit
- GIVEN `status --provider openai-codex` is requested
- WHEN status output is rendered
- THEN the provider status MUST be scoped to openai-codex and MUST not imply Anthropic status
