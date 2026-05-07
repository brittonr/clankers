## Phase 1: Service extraction

- [x] [serial] Write the prompt assembly service OpenSpec package. [covers=prompt-assembly.service] [evidence=openspec validate extract-prompt-assembly-service --strict]
- [ ] [serial] Introduce a reusable prompt assembly service/API with explicit policy and host context inputs. [covers=prompt-assembly.service]
- [ ] [parallel] Add no-filesystem host-context-only tests. [covers=prompt-assembly.service.host-context-only]
- [ ] [parallel] Add safe provenance metadata and redaction tests. [covers=prompt-assembly.provenance]

## Phase 2: Clankers parity

- [ ] [serial] Route normal Clankers prompt assembly through the service or add parity fixtures proving identical section order. [covers=prompt-assembly.service.clankers-parity]
- [ ] [parallel] Add context-reference policy tests for disabled and unsupported embedding modes. [covers=prompt-assembly.context-reference-boundary.disabled]
- [ ] [parallel] Document prompt assembly policy knobs for embedders. [covers=prompt-assembly.service]
