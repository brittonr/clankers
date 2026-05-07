## Phase 1: Specification foundation

- [x] [serial] Define runtime extension service matrix testing scope.

## Phase 2: Matrix fixture and observers

- [ ] [serial] Define runtime service matrix axes for auth, credential-pool, provider/router, plugin, MCP/gateway-placeholder availability, and failure mode.
- [ ] [depends:service-axes] Add fake service observers and filesystem/socket sentinels for hidden side-effect detection.
- [ ] [depends:service-axes] Add mixed injected/absent runtime matrix cases and safe-receipt redaction assertions.

## Phase 3: Verification rails

- [ ] [depends:service-matrix] Add a freshness checker for service-axis coverage and critical mixed-service cases.
- [ ] [depends:service-matrix] Wire runtime service matrix checks into embedded SDK acceptance.
- [ ] [serial] Run focused runtime service tests and embedded SDK acceptance; archive, commit, and push.
