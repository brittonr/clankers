## MODIFIED Requirements

### Requirement: Transcript compatibility is explicit [r[sdk-message-contract-boundary.transcript-compat-feature]]

Clankers transcript compatibility records MAY remain public only through an explicit compatibility module or feature that documents ownership by desktop/session/provider/controller adapters.

#### Scenario: compatibility APIs carry owners and fixtures [r[sdk-message-contract-boundary.transcript-compat-feature.owner-fixtures]]
- GIVEN a transcript, provider, session, or legacy import API is labeled optional support or compatibility-only
- WHEN SDK inventory and boundary rails validate the API
- THEN the API MUST name its owning adapter or feature boundary and have a serialization or behavior fixture
- AND default green SDK examples and root exports MUST NOT require that compatibility API unless the example explicitly opts in
