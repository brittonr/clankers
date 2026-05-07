## ADDED Requirements

### Requirement: Auth Runtime Extension Access [r[tool-host-embedding.auth-runtime-extension-access]]

The system MUST route embedded auth-store lookup and credential-pool selection through explicit runtime extension services when those services are used by an embedding host.

#### Scenario: Default-safe auth services [r[tool-host-embedding.auth-runtime-extension-access.default-safe]]

- GIVEN an embedded/default runtime without injected auth services
- WHEN auth-store lookup, pending login verifier access, refresh persistence, or credential-pool selection is requested
- THEN the operation MUST fail closed without reading auth files, writing verifier state, refreshing tokens, or persisting credentials

#### Scenario: Injected auth lookup receipt [r[tool-host-embedding.auth-runtime-extension-access.injected-lookup]]

- GIVEN a host-injected auth-store snapshot with provider/account entries
- WHEN a runtime auth lookup is requested
- THEN the service MUST return a safe receipt containing provider/account/status/count/kind metadata and MUST NOT include credential values, refresh tokens, verifier contents, headers, environment values, or raw auth-file contents

#### Scenario: Injected credential-pool selection receipt [r[tool-host-embedding.auth-runtime-extension-access.pool-selection]]

- GIVEN a host-injected auth-store snapshot and credential-pool strategy request
- WHEN runtime credential-pool selection is requested
- THEN the service MUST select from injected entries using safe provider/account/strategy metadata and MUST NOT start OAuth flows, refresh credentials, or expose credential values
