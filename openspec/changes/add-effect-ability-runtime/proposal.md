## Why

Clankers tools, subagents, plugins, browser access, shell commands, secrets, network, and scheduler actions are effectful capabilities. Today those policies are distributed across catalog construction, tool dispatch, permission checks, runtime services, and daemon/subagent paths. Unison's abilities model suggests making effects explicit interfaces with swappable handlers. Unison's remote computation model also suggests content-hash dependency sync for remote/subagent execution.

## What Changes

- **Ability-style effect interfaces**: Define typed effect classes for file, shell, network, secret, browser, scheduler, provider, plugin, and delivery actions.
- **Handlers and simulation**: Route effect requests through handlers that can allow, deny, replay, simulate, or record effects.
- **Remote dependency sync**: Let subagents/remote daemons declare required skills/tools/prompts/manifests by content hash and fetch missing safe artifacts before execution.

## Capabilities

### New Capabilities
- `effect-ability-runtime`: Typed effect requests, handler policies, replay/simulation, and remote dependency sync.

### Modified Capabilities
- `tool-host-embedding`: Capability packs map to effect abilities and handlers.
- `embeddable-agent-engine`: Engine effects can be executed by host-provided handlers.

## Impact

- **Files**: tool dispatch traits, runtime service adapters, daemon/subagent protocol, tests.
- **APIs**: typed effect request/result envelopes and handler traits.
- **Dependencies**: no new remote service required; sync rides existing daemon/QUIC paths where available.
- **Testing**: allow/deny/simulate/replay matrix; remote missing-dependency sync; fail-closed tests.
