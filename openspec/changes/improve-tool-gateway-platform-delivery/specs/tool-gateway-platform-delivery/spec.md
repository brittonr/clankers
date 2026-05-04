## ADDED Requirements

### Requirement: Unified Toolset Policy [r[tool-gateway.toolsets]]
The system MUST evaluate enabled toolsets, disabled tools, capability ceilings, and mode restrictions through one shared policy path.

#### Scenario: Standalone and daemon agree [r[tool-gateway.toolsets.scenario.standalone-and-daemon-agree]]
- GIVEN the same settings and command-line toolset choices are used in standalone and daemon sessions
- WHEN tools are built
- THEN both modes expose the same allowed tool names except for explicitly documented transport limitations

#### Scenario: Runtime changes propagate [r[tool-gateway.toolsets.scenario.runtime-changes-propagate]]
- GIVEN a user disables or enables tools during an attached session
- WHEN the command is accepted
- THEN local UI state and daemon state converge without duplicate noisy acknowledgements

### Requirement: Platform-Aware Delivery Receipts [r[tool-gateway.delivery]]
The system MUST route generated files, media, and scheduled-job outputs through platform-aware delivery adapters with safe receipts.

#### Scenario: Media delivery receipt [r[tool-gateway.delivery.scenario.media-delivery-receipt]]
- GIVEN a tool produces a file or media artifact for a platform session
- WHEN delivery runs
- THEN clankers records artifact type, safe path or platform handle, status, and error class without tokens or raw destination secrets
