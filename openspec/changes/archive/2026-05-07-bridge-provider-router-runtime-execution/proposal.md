# Bridge provider router runtime execution

## Why
Plugin execution now proves runtime extension execution for tools. Provider/router execution is the next high-value seam because model calls are the core embedding dependency and currently risk hidden desktop assumptions such as daemon autostart, auth verifier writes, or provider-specific request shaping outside the runtime boundary.

## What Changes
- Route a real desktop provider/router execution path through `RuntimeServices.extensions.provider_router` so embedders can inject model execution explicitly instead of relying on ambient daemon/router/OAuth discovery.
- Add neutral provider execution inputs sufficient for a focused prompt/model call.
- Add a desktop adapter path that executes through an injected provider/router implementation.
- Preserve fail-closed behavior when no provider/router is injected.
- Return sanitized receipts with counts/status only, not prompts, raw provider payloads, headers, tokens, or model output.
