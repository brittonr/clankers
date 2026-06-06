## MODIFIED Requirements

### Requirement: Runtime facade classification is explicit [r[remaining-coupling-drain.runtime-facade-classification]]

`clankers-runtime` MUST be classified as yellow-only, a documented green-subset facade, or a split set of green/yellow owners before new runtime APIs are advertised as embedded SDK contracts.

#### Scenario: steel orchestration contracts split from execution policy [r[remaining-coupling-drain.runtime-facade-classification.steel-contract-owner]]
- GIVEN Steel turn planning, host-call, or repo-evolution data is useful to embedded hosts
- WHEN those public APIs are reviewed for SDK promotion
- THEN serializable orchestration contracts MUST live in a neutral owner or be explicitly labeled yellow runtime adapter surface
- AND executable script loading, host-call dispatch, repo mutation, filesystem receipts, and Nickel/profile resolution MUST remain outside the green owner
