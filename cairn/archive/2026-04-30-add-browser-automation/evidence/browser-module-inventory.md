# Browser Automation Module Inventory

## Existing capability

- `src/tools/web.rs` owns the current stateless web surface: Kagi search and HTTP fetch/content extraction. It does not navigate, click, fill forms, keep cookies, or expose page state.
- Existing browser references are limited to auth URL launching (`src/slash_commands/handlers/auth.rs`, `src/commands/auth.rs`, router auth) and UI chrome naming; there is no CDP/WebDriver/Playwright/browser automation backend.
- Anthropic direct-browser-access headers appear in provider/router code, but that is provider transport behavior, not an agent-visible stateful browser tool.

## Proposed ownership

- `crates/clankers-config/src/settings.rs`: add `browserAutomation` settings and validation. This matches the recently-added `mcp` settings pattern: typed config, serde camelCase, explicit validation errors, default disabled/empty behavior, and deep merge through existing `Settings::merge_layers`.
- `src/tools/browser.rs`: add the agent-visible browser automation tool adapter. It should implement `Tool`, expose a compact action schema (`navigate`, `click`, `fill`, `evaluate`, `screenshot`, `snapshot`, etc. as the supported slice evolves), normalize backend results into `ToolResult`, and attach metadata in `ToolResult.details`.
- `src/tools/mod.rs`: publish the new module.
- `src/modes/common.rs`: register the browser tool as `ToolTier::Specialty` when settings enable it and when config validation says a safe backend is available. This is the shared path for prompt/headless, TUI, and daemon/session modes.
- `src/modes/agent_setup.rs` and `src/modes/event_loop_runner/key_handler.rs`: likely no special browser wiring beyond passing `Settings` through `ToolEnv`; review after implementation for any new runtime handles.
- Session persistence/replay remains in the existing tool-result path (`src/modes/event_loop_runner/mod.rs`, provider/message storage). Browser-specific audit data should be normalized into `ToolResult.details` rather than backend-specific blobs.
- `README.md` and `docs/src/reference/config.md`: document the tool and first-pass local CDP configuration.

## First-pass backend boundary

- Prefer a trait-backed adapter so tests can use a fake backend without launching Chromium.
- Local Chrome/Chromium CDP is the intended first real backend, but the first implementation may land the config/tool/runtime seam before implementing every CDP operation.
- Remote providers (Browserbase, Browser Use, hosted browsers), persistent profiles beyond a controlled user-data-dir, downloads, file uploads, CAPTCHA/payment flows, and arbitrary credential injection should be explicit follow-up/non-goals unless separately specified.

## Safety/policy notes

- Browser automation crosses network, filesystem profile, and credential boundaries; validation should reject ambiguous/unsafe configuration with actionable errors.
- Do not silently fall back from missing browser/CDP config to stateless `web` fetch.
- Metadata must redact secrets and should include source `browser_automation`, backend identity, action, URL/origin when safe, elapsed/status, and redacted error details.
