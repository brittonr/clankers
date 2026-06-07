## MODIFIED Requirements

### Requirement: Runtime facade classification is explicit [r[remaining-coupling-drain.runtime-facade-classification]]

`clankers-runtime` MUST be classified as yellow-only, a documented green-subset facade, or a split set of green/yellow owners before new runtime APIs are advertised as embedded SDK contracts.

#### Scenario: runtime adapter shells are separately classified [r[remaining-coupling-drain.runtime-facade-classification.adapter-shell-buckets]]
- GIVEN `clankers-runtime` exposes contracts and desktop service implementations
- WHEN runtime facade inventory is generated
- THEN each public API group MUST be classified as green contract, yellow host-injection surface, or desktop adapter shell
- AND desktop adapter shell groups MUST NOT be advertised as default embedded SDK contracts
