# Tasks

- [ ] V1 [covers=lifecycle.strong-constraints] Verify generated artifact hygiene with `scripts/check-generated-artifact-hygiene.rs` and checked-in evidence.
- [ ] V2 [covers=lifecycle.local-verification] Verify local verification contract coverage with `nix run .#cairn -- validate --root .` and the positive-strong-constraint-spec-coverage fixture.
- [ ] V3 [covers=lifecycle.no-github-delivery] Verify forbidden GitHub delivery path with the positive-strong-constraint-spec-coverage fixture and local evidence bundle.
