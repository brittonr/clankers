# Design: harden spec omission gate

## Scope

This change extends the existing deterministic review-gate fixture runner with a spec-stage category table. The gate remains credential-free and local-only.

## Trigger and satisfaction model

Spec-stage categories trigger from `proposal.md` plus `design.md`, not from `spec.md` itself. This prevents an empty or vague spec from satisfying its own obligation. A category is satisfied only when `spec.md` contains the expected concrete scenario terms.

Initial categories:

- `missing-omitted-provider-default-spec`: proposal/design promises omitted-provider Anthropic defaults, but specs do not encode omitted provider + Anthropic default behavior.
- `missing-malformed-account-claim-spec`: proposal/design promises missing or malformed `chatgpt_account_id` claim handling, but specs do not encode malformed claim behavior.
- `missing-provider-scoped-status-spec`: proposal/design promises explicit provider-scoped status behavior, but specs do not encode status + provider + `openai-codex` behavior.

## Fixtures

- Negative fixture: proposal/design require all three contracts while `spec.md` only has a broad provider-login requirement.
- Positive fixture: `spec.md` contains explicit scenarios for omitted provider defaults, malformed claim handling, and provider-scoped status.

## Safety

Fixtures are sanitized Markdown and contain no secrets, credentials, live requests, or provider payloads.
