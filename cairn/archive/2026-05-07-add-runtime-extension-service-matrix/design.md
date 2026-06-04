## Context

Recent changes routed plugin, provider/router, and auth through injected runtime services. The remaining risk is cross-service coupling: a provider request should not trigger auth-file reads unless an auth service is explicitly injected, and plugin publication should not start provider/router or credential services.

## Goals / Non-Goals

**Goals:** matrix-test fail-closed default behavior, injected service behavior, mixed combinations, receipt redaction, and no hidden side effects.

**Non-Goals:** live provider calls, real OAuth, or real plugin subprocess lifecycle beyond existing focused runtime tests.

## Decisions

### 1. Service presence is a matrix axis

**Choice:** enumerate absent, fake-injected-success, fake-injected-error, and denied service states for auth, provider/router, and plugin runtime surfaces.

**Rationale:** mixed combinations catch accidental ambient fallback and hidden discovery.

### 2. Side effects are observed by counters and filesystem sentinels

**Choice:** instrument fake services plus sentinel paths for auth files, login verifiers, sockets, and runtime startup attempts.

**Rationale:** negative safety claims need proof that nothing was touched, not just that an error was returned.

### 3. Redaction is asserted uniformly

**Choice:** every matrix receipt assertion checks that prompts, credentials, headers, raw auth files, raw plugin args/output, environment values, and raw provider bodies are absent.

**Rationale:** safe receipts are a cross-cutting contract across all extension services.
