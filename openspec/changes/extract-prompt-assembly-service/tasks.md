## Phase 1: Service extraction

- [x] [serial] Write the prompt assembly service OpenSpec package. [covers=prompt-assembly.service] [evidence=openspec validate extract-prompt-assembly-service --strict]
- [x] [serial] Introduce a reusable prompt assembly service/API with explicit policy and host context inputs. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=prompt-assembly.service] [evidence=clankers_runtime::PromptAssembler]
- [x] [parallel] Add no-filesystem host-context-only tests. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=prompt-assembly.service.host-context-only] [evidence=clankers-runtime::tests::prompt_assembly_rejects_filesystem_discovery_when_disabled]
- [x] [parallel] Add safe provenance metadata and redaction tests. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=prompt-assembly.provenance] [evidence=clankers-runtime::tests::prompt_assembly_host_context_only_redacts_provenance_content]

## Phase 2: Clankers parity

- [ ] [serial] Route normal Clankers prompt assembly through the service or add parity fixtures proving identical section order against the current prompt assembly path. [covers=prompt-assembly.service.clankers-parity]
- [ ] [parallel] Add context-reference policy tests for disabled and unsupported embedding modes with structured unsupported-reference metadata. [covers=prompt-assembly.context-reference-boundary.disabled]
- [x] [parallel] Document prompt assembly policy knobs for embedders. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=prompt-assembly.service] [evidence=docs/src/reference/embedding.md]
