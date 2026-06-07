## ADDED Requirements

### Requirement: Remote auth UX docs are authoritative [r[ucan-basalt-daemon-auth.remote-auth-ux-docs]]

Clankers MUST provide operator-facing documentation for remote daemon authentication that describes public UCAN credentials, Basalt policy admission, delegation, revocation, remote attach/chat/Matrix use, and safe receipt/redaction behavior.

#### Scenario: Reference guide covers public credential workflow [r[ucan-basalt-daemon-auth.remote-auth-ux-docs.reference-guide]]
- GIVEN an operator needs to grant remote daemon access
- WHEN they read the remote-auth reference guide
- THEN the guide MUST explain how to create a public UCAN credential for a remote daemon audience
- AND it MUST explain how to delegate from a parent credential without exposing compact token bodies
- AND it MUST describe revocation or rotation as the supported way to remove authority

#### Scenario: Entry points distinguish public UCAN from legacy local compatibility [r[ucan-basalt-daemon-auth.remote-auth-ux-docs.entrypoints]]
- GIVEN an operator reads the README, getting-started auth page, or daemon reference page
- WHEN the docs mention remote daemon capability tokens
- THEN they MUST identify remote auth as public UCAN plus Basalt admission
- AND they MUST NOT present legacy `clanker-auth` credentials as the default remote verifier path

#### Scenario: Basalt source boundary is documented [r[ucan-basalt-daemon-auth.remote-auth-ux-docs.basalt-source]]
- GIVEN a developer or operator audits dependency sources
- WHEN they read the remote-auth docs
- THEN the docs MUST state that local Cargo development uses `../basalt`
- AND they MUST state that Nix builds map that path to the pinned `OnixResearch/basalt` flake input through `externalSources`

#### Scenario: Docs contract rail prevents drift [r[ucan-basalt-daemon-auth.remote-auth-ux-docs.contract-rail]]
- GIVEN the docs contain remote-auth command examples and security wording
- WHEN the deterministic docs/help contract test runs
- THEN it MUST verify that examples use clap-accurate flags for token creation and remote attach
- AND it MUST verify that docs mention public UCAN plus Basalt for remote auth
- AND it MUST fail if docs embed raw compact UCAN token strings, signing keys, auth JSON, or legacy-token remote guidance

#### Scenario: Closeout validation covers remote auth docs [r[ucan-basalt-daemon-auth.remote-auth-ux-docs.closeout]]
- GIVEN the remote-auth UX docs change is ready to close
- WHEN validation runs
- THEN the docs contract rail, public UCAN boundary test, Cairn proposal/design/tasks gates, Cairn validation, and diff checks MUST pass
