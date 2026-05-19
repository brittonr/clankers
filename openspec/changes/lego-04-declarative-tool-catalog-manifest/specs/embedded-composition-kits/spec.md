## MODIFIED Requirements

### Requirement: Tool catalog manifest contract [r[embedded-composition-kits.tool-catalog-manifest]]

The system MUST extend this requirement with the next lego-readiness slice.

#### Scenario: Manifest export is normalized and runtime-neutral [r[embedded-composition-kits.tool-catalog-manifest.manifest-export-is-normalized-and-runtime-neutral]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN A product-owned manifest exports deterministic EmbeddedToolCatalog-compatible metadata with no stdio startup, Extism loading, network calls, secret reads, or product tool execution.

#### Scenario: Manifest validation diagnostics are actionable [r[embedded-composition-kits.tool-catalog-manifest.manifest-validation-diagnostics-are-actionable]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN Duplicate names, invalid schemas, unknown runtime kinds, unsafe capability defaults, missing redaction, and undeclared dangerous capabilities fail closed with typed diagnostics.

#### Scenario: Normalized evidence distinguishes semantic drift [r[embedded-composition-kits.tool-catalog-manifest.normalized-evidence-distinguishes-semantic-drift]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN The acceptance rail hashes authored manifests, normalized metadata, denial fixtures, and truncation fixtures so semantic policy changes are visible while harmless formatting can avoid fixture churn.
