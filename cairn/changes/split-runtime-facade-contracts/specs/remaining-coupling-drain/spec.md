## ADDED Requirements

### Requirement: Runtime facade contracts split by authority [r[remaining-coupling-drain.runtime-facade-contract-split]]

`clankers-runtime` MUST separate reusable green contracts from yellow host-injection surfaces and desktop adapter shells so public runtime exports do not imply ambient provider, auth, plugin, process, prompt, session, storage, clock, or executable Steel authority.

#### Scenario: Runtime public exports are inventoried [r[remaining-coupling-drain.runtime-facade-contract-split.inventory]]
- GIVEN runtime facade public exports are reviewed
- WHEN classification validation runs
- THEN every touched export MUST be classified as green contract, yellow host-injection surface, or desktop adapter shell
- AND the inventory MUST include source hashes, dependency summaries, and deterministic owner labels

#### Scenario: Green contracts have no ambient authority [r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts]]
- GIVEN a runtime DTO or helper is advertised as a green reusable contract
- WHEN it is compiled or tested without desktop services
- THEN it MUST NOT require provider, auth, plugin, process, prompt filesystem, skill, session, storage, clock, Nickel, Steel execution, daemon, TUI, or global path access
- AND executable policy MUST remain in yellow or desktop adapter owners

#### Scenario: Runtime defaults fail closed [r[remaining-coupling-drain.runtime-facade-contract-split.fail-closed-services]]
- GIVEN an embedded host creates runtime defaults without service injection
- WHEN provider, auth, plugin, process, prompt filesystem, skill, session, storage, or executable Steel behavior is requested
- THEN runtime MUST return typed unavailable diagnostics
- AND it MUST NOT probe global/project config, auth files, daemon sockets, plugin directories, or desktop session stores

#### Scenario: SDK docs follow classification [r[remaining-coupling-drain.runtime-facade-contract-split.docs]]
- GIVEN generated SDK/API docs are refreshed
- WHEN runtime exports are listed
- THEN green contracts MUST be separated from yellow host-injection surfaces and desktop adapter shells
- AND desktop adapter shell groups MUST NOT be advertised as default embedded SDK contracts

#### Scenario: Runtime facade validation runs [r[remaining-coupling-drain.runtime-facade-contract-split.validation]]
- GIVEN runtime facade contracts are split or reclassified
- WHEN focused validation runs
- THEN public API inventory rails, fail-closed tests, Steel contract split fixtures, and SDK/lego docs rails MUST pass

#### Scenario: Runtime facade closeout is gated [r[remaining-coupling-drain.runtime-facade-contract-split.closeout]]
- GIVEN the runtime facade split is ready to close
- WHEN closeout validation runs
- THEN affected cargo checks, Cairn gates, Cairn validation, and diff checks MUST pass
