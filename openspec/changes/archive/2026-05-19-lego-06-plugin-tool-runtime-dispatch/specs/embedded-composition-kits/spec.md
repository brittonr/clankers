## MODIFIED Requirements

### Requirement: Plugin/tool runtime separation [r[embedded-composition-kits.plugin-tool-runtime-separation]]

The system MUST keep tool runtime kinds swappable behind explicit contracts without sending one runtime kind through another runtime loader.

#### Scenario: Runtime kind dispatch is explicit [r[embedded-composition-kits.plugin-tool-runtime-separation.dispatch]]

- GIVEN a catalog or plugin manifest declares a runtime kind such as Extism, stdio, built-in, or product-owned executor

- WHEN discovery and execution planning run

- THEN only the matching runtime loader/executor MAY receive that entry
- THEN non-Extism entries MUST NOT flow through eager WASM loading or produce bogus missing-WASM errors

#### Scenario: Launch policy is contract checked [r[embedded-composition-kits.plugin-tool-runtime-separation.nickel-policy]]

- GIVEN runtime manifests include launch policy, sandbox requirements, capability requirements, and redaction policy

- WHEN manifest policy is checked

- THEN Nickel contracts SHOULD validate runtime-kind allowlists, required fields per kind, and bounded exceptions before runtime dispatch
- THEN generic SDK crates MUST consume typed manifest data rather than depending on Nickel evaluation at execution time

#### Scenario: Dispatch matrix evidence is content addressed [r[embedded-composition-kits.plugin-tool-runtime-separation.blake3-matrix]]

- GIVEN tests cover Extism, stdio, built-in, and product-owned tool entries

- WHEN the acceptance rail records runtime dispatch evidence

- THEN it SHOULD include BLAKE3 hashes for normalized manifests, runtime-kind allowlist exports, and dispatch matrix fixtures
- THEN changing the runtime-kind contract MUST require an intentional update to tests, docs, and receipt evidence
