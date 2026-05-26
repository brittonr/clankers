# Tasks

- [ ] [serial] T1. Create the docs evidence page with tag target, payload commit, harness run id, and local receipt paths. [covers=r[dogfood-full-readiness-evidence-page.checkpoint-binding],r[dogfood-full-readiness-evidence-page.harness-index]]
- [ ] [parallel] T2. Add dogfood receipt facts and explicit readiness scope boundary. [covers=r[dogfood-full-readiness-evidence-page.dogfood-facts],r[dogfood-full-readiness-evidence-page.scope-boundary]]
- [ ] [parallel] T3. Link the page from docs navigation and any release-readiness index surface. [covers=r[dogfood-full-readiness-evidence-page.checkpoint-binding]]
- [ ] [serial] T4. Verify with `mdbook build docs`, focused content checks, and `git diff --check`. [covers=r[dogfood-full-readiness-evidence-page.harness-index],r[dogfood-full-readiness-evidence-page.dogfood-facts],r[dogfood-full-readiness-evidence-page.scope-boundary]]
