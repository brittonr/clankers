# Design

## Runtime contract
`ProviderExecutionRequest` remains transport-neutral and gains optional prompt/system/max-token/session metadata for a narrow model execution. The public runtime crate does not depend on `clankers-provider` or daemon protocol types.

## Desktop adapter
`DesktopRuntimeServiceAdapters` accepts an explicit provider/router object. The adapter builds a normal `clankers_provider::CompletionRequest`, executes it through the injected provider, drains stream events, and returns a sanitized `ExtensionReceipt` containing provider/model/session identifiers and event/output byte counts only.

Without injection, provider execution fails closed with `ExtensionUnavailable` and does not start a daemon, trigger OAuth, or touch auth verifier/runtime files.

## Safety
Receipts must not include raw prompts, system prompts, provider request bodies, model output text, tool payloads, headers, tokens, environment values, or credential material.
