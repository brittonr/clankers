## Tasks

- [ ] [serial] V1: Run fixture `fixtures/codex/default_override_request_shape.json` for default/override verbosity request shape [covers=openai-codex.request-shape]
- [ ] [serial] V2: Run helper `assert_requested_account_active_after_login` for active account persistence [covers=openai-codex.auth-active-account]
- [ ] [serial] V3: Run fixture `fixtures/codex/entitlement_probe_retry_refresh.json` for entitlement probe retry and 401 refresh-retry probe headers [covers=openai-codex.entitlement-probe]
- [ ] [serial] V4: Run command `cargo nextest run codex_function_call_arguments_delta_fixture` for `function_call_arguments.delta` tool-call delta stream boundary coverage [covers=openai-codex.stream-boundary]
