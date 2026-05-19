## MODIFIED Requirements

### Requirement: Plugin/tool runtime separation [r[embedded-composition-kits.plugin-tool-runtime-separation]]

The system MUST extend this requirement with the next lego-readiness slice.

#### Scenario: Runtime kind dispatch is single-owner [r[embedded-composition-kits.plugin-tool-runtime-separation.runtime-kind-dispatch-is-single-owner]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN Discovery and execution planning send each manifest entry only to the matching runtime loader or executor; non-Extism entries never flow through eager WASM loading.

#### Scenario: Launch policy is validated before dispatch [r[embedded-composition-kits.plugin-tool-runtime-separation.launch-policy-is-validated-before-dispatch]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN Runtime manifests declare launch policy, sandbox requirements, capability requirements, and redaction policy that are checked before runtime dispatch.

#### Scenario: Dispatch matrix evidence is content addressed [r[embedded-composition-kits.plugin-tool-runtime-separation.dispatch-matrix-evidence-is-content-addressed]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN Acceptance evidence hashes normalized manifests, runtime-kind allowlist exports, and dispatch matrix fixtures for Extism, stdio, built-in, and product-owned entries.
