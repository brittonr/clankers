## ADDED Requirements

### Requirement: Tool catalog manifest contract [r[embedded-composition-kits.tool-catalog-manifest]]

The system MUST support a product-owned tool catalog manifest that validates tools before they are exposed to an embedded agent.

#### Scenario: Manifest exports runtime-neutral metadata [r[embedded-composition-kits.tool-catalog-manifest.export]]

- GIVEN a product authors a declarative tool catalog manifest
- WHEN the manifest is validated and exported
- THEN it MUST produce runtime-neutral embedded tool metadata compatible with `EmbeddedToolCatalog`
- THEN export MUST NOT start stdio processes, load Extism modules, perform network calls, open secrets, or execute product tools

#### Scenario: Manifest policy fails closed [r[embedded-composition-kits.tool-catalog-manifest.fail-closed]]

- GIVEN a catalog contains duplicate names, invalid schemas, unknown runtime kinds, unsafe defaults, missing redaction policy, or undeclared dangerous capabilities
- WHEN validation runs
- THEN it MUST return typed errors before metadata is visible to an agent turn
- THEN denied fields MUST NOT be silently dropped or widened

#### Scenario: Catalog evidence is normalized and hashed [r[embedded-composition-kits.tool-catalog-manifest.blake3-evidence]]

- GIVEN the acceptance rail validates catalog fixtures
- WHEN it records catalog evidence
- THEN it MUST include BLAKE3 hashes for authored manifests, normalized exported metadata, denial fixtures, and truncation fixtures
- THEN non-semantic formatting changes MAY avoid changing normalized metadata hashes, but semantic policy changes MUST change the evidence

#### Scenario: Nickel remains an authoring boundary [r[embedded-composition-kits.tool-catalog-manifest.nickel-authoring]]

- GIVEN Nickel is used to author the first-class catalog manifest
- WHEN embedded Rust crates load catalog data
- THEN Nickel SHOULD provide author-time contracts and exported fixture data
- THEN generic SDK crates MUST NOT require Nickel evaluation, filesystem policy files, or shell commands at runtime
