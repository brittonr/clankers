## MODIFIED Requirements

### Requirement: Opt-in live readiness tests [r[release-readiness.live-nextest]]
The repository MUST represent live local-model readiness as Rust integration tests runnable by `cargo nextest` with explicit opt-in gates, short availability probes, bounded generation timeouts, and no implicit OAuth/browser login flows. For Clankers testing, dogfood, and release-readiness slices that require a live model, qwen on aspen2 MUST be the primary live test model path unless a task explicitly scopes a different provider.

#### Scenario: Qwen on aspen2 is the primary live testing model [r[release-readiness.live-nextest.qwen-aspen2-primary]]
- GIVEN a Clankers testing, dogfood, or release-readiness slice needs live model evidence
- WHEN an operator follows the release-readiness documentation or harness inventory
- THEN the documented primary live model path SHALL be qwen on aspen2 through the `aspen2-qwen36` harness selector
- AND OpenAI OAuth/Codex-backed checks SHALL NOT be substituted for this live testing path unless the task explicitly requests that provider

#### Scenario: Live readiness runs against configured local model [r[release-readiness.live-nextest.local-model]]
- GIVEN live readiness is explicitly enabled and a configured OpenAI-compatible local model endpoint is available
- WHEN the nextest live readiness filter runs
- THEN the test sends a bounded request through the routed provider path
- AND it asserts a deterministic completion or stream-shape contract

#### Scenario: Live readiness is explicit when unavailable [r[release-readiness.live-nextest.unavailable]]
- GIVEN live readiness is not explicitly enabled or the configured model endpoint is unavailable
- WHEN the live readiness test is discovered
- THEN it reports an explicit skip/prerequisite message
- AND it does not mark the endpoint as verified
