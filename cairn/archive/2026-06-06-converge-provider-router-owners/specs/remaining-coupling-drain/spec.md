## MODIFIED Requirements

### Requirement: Provider/router compatibility converges to one owner per concern [r[remaining-coupling-drain.provider-router-convergence]]

Provider-native request shaping, model/account discovery, auth refresh/probing, routing/fallback/cooldown, retry behavior, and stream normalization MUST each have one owner. Compatibility layers MUST translate DTOs only and MUST NOT duplicate policy.

#### Scenario: provider concerns have owner receipts [r[remaining-coupling-drain.provider-router-convergence.concern-owner-map]]
- GIVEN provider/router code owns request shaping, auth, discovery, routing, retry, cache-key, or stream-normalization behavior
- WHEN provider-router boundary validation inventories that behavior
- THEN each concern MUST name exactly one policy owner and any compatibility adapter MUST name the projection helper or backend entrypoint it delegates to
- AND duplicate policy in compatibility adapters MUST fail unless it is a documented temporary convergence row with a focused fixture

#### Scenario: compatibility adapters delegate policy [r[remaining-coupling-drain.provider-router-convergence.adapter-delegation]]
- GIVEN `clankers-provider` compatibility code calls `clanker-router` or a routed backend
- WHEN request, cache-key, retry, auth-probe, or stream behavior changes
- THEN compatibility code MUST translate DTOs, stream events, and errors only
- AND provider-native body construction, routing/fallback/cooldown, auth probe, retry, and stream state-machine policy MUST remain in the declared owner module with parity fixtures
