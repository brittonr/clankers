## Context

This change tracks Hermes feature-parity work for Browser Automation. Clankers already has strong Rust-native agent, daemon, plugin, routing, scheduling, and tool foundations; this change should compose with those foundations rather than bypass them.

## Goals / Non-Goals

**Goals:**
- Provide a small, testable first implementation that is useful from the TUI, prompt mode, and daemon/session paths.
- Keep policy decisions explicit: credentials, sandboxing, persistence, and output delivery must be auditable.
- Document gaps intentionally left for follow-up.

**Non-Goals:**
- Large rewrites of the agent loop or provider stack unless required by the capability boundary.
- Hidden best-effort behavior that silently drops outputs, credentials, or session context.

## Decisions

### 1. Build on existing clankers primitives

**Choice:** Reuse existing tool registration, daemon/session persistence, config paths, provider routing, and plugin/runtime abstractions where possible.

**Rationale:** This keeps the feature consistent with clankers architecture and avoids Hermes-shaped islands that are hard to maintain.

**Alternative:** Copy Hermes behavior directly as a separate subsystem. Rejected because duplicated lifecycle and policy handling would drift quickly.

**Implementation:** Add the minimum new module/crate surface needed for Browser Automation, then wire it through the existing CLI/TUI/daemon paths.

### 2. Make policy and observability first-class

**Choice:** Every implementation MUST expose enough state for tests, logs, session replay, and user-facing errors.

**Rationale:** These features often cross process, network, or file boundaries. Silent fallback is harder to debug than a clear unsupported-path error.

**Alternative:** Optimize only for a happy-path demo. Rejected because these are agent autonomy features and failures must be recoverable.

### 3. User-facing surface

**Choice:** Add a single agent-visible `browser` tool, published as a Specialty tool when `browserAutomation.enabled = true`. The tool exposes explicit actions rather than natural-language browser instructions:

- `navigate`: open or reuse a session and navigate to a URL.
- `snapshot`: return normalized page state such as URL, title, visible text excerpt, and known selectors.
- `click`: click a selector or accessible label.
- `fill`: fill a selector or accessible label with text.
- `evaluate`: run a constrained JavaScript expression only when `allowEvaluate = true`.
- `screenshot`: return screenshot metadata/path or image content when supported.
- `close`: close one browser session.

All actions accept an optional `sessionId`; if omitted, clankers uses a default session scoped to the agent session.

**Rationale:** A single tool mirrors Hermes browser automation capability while keeping schema overhead bounded. Explicit actions are easier to test, replay, and policy-gate than freeform instructions.

**Alternative:** Add separate `browser_navigate`, `browser_click`, etc. tools. Rejected for the first pass because it expands the always-advertised tool surface and complicates shared session state.

### 4. Configuration surface

**Choice:** Add `Settings::browser_automation` serialized as `browserAutomation`:

```json
{
  "browserAutomation": {
    "enabled": true,
    "backend": "cdp",
    "cdpUrl": "http://127.0.0.1:9222",
    "browserBinary": "chromium",
    "userDataDir": ".clankers/browser-profile",
    "headless": true,
    "allowEvaluate": false,
    "allowScreenshots": true,
    "timeoutMs": 30000,
    "allowedOrigins": ["http://localhost:*", "https://example.com"]
  }
}
```

Validation rejects enabled CDP config without either `cdpUrl` or `browserBinary`, blank origin entries, non-positive timeouts, `userDataDir` values that resolve outside the project/global clankers profile boundary unless explicitly absolute, and screenshot/evaluate requests disabled by policy.

**Rationale:** Hermes supports several browser backends; clankers should start with local CDP but shape config for future backends without breaking users.

### 5. First-pass backend and unsupported cases

**Choice:** The first implementation uses a trait-backed local CDP adapter boundary. Unit/integration tests use a fake backend and do not require Chromium. Runtime execution returns actionable unsupported/configuration errors when no usable local CDP endpoint or browser binary is configured.

**Unsupported in the first pass:** Browserbase/Browser Use/remote hosted providers, captcha solving, payments, downloading arbitrary files, uploading local files, credential injection, cross-session cookie sharing outside the configured profile, unrestricted JavaScript evaluation, and silent fallback to stateless `web` fetch.

**Rationale:** This lands a production-ready seam and safe tool semantics before committing to every provider-specific behavior.

## Risks / Trade-offs

**Scope creep** → Start with a minimal backend/API and document additional backends as future tasks.

**Security regressions** → Reuse sanitized environments, capability checks, and explicit allowlists.

**Session replay drift** → Store normalized events/metadata rather than backend-specific blobs when possible.
