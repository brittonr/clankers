## Phase 1: Agent port boundary

- [x] [serial] I1: Inventory concrete `clankers-agent` dependencies by service family and mark each as reusable policy, compatibility adapter, or app-edge shell. r[sdk-agent-port-boundary.inventory] [covers=sdk-agent-port-boundary.inventory]
- [x] [serial] I2: Define or select narrow agent-shell ports for model execution, prompt/config, storage/search, hooks, skills, cost, and cancellation. r[sdk-agent-port-boundary.ports.explicit-services] [covers=sdk-agent-port-boundary.ports.explicit-services]
- [x] [serial] I3: Move at least one concrete dependency family out of reusable turn modules and into a desktop/runtime adapter. r[sdk-agent-port-boundary.ports.adapter-owned] [covers=sdk-agent-port-boundary.ports.adapter-owned]
- [x] [parallel] I4: Update lego/SDK docs and owner receipts so `clankers-agent` remains yellow shell and new concrete imports need an owner. r[sdk-agent-port-boundary.rails.owner-receipts] [covers=sdk-agent-port-boundary.rails.owner-receipts]

## Phase 2: Verification

- [x] [serial] V1: Add focused tests proving the migrated port preserves model/tool/stream terminal behavior against the compatibility adapter. r[sdk-agent-port-boundary.verification.parity] [covers=sdk-agent-port-boundary.verification.parity] [evidence=evidence/agent-port-parity.md]
- [x] [serial] V2: Add or update source/dependency rails rejecting new concrete provider/config/DB/hook/skill/model-selection imports in reusable turn modules without an owner receipt. r[sdk-agent-port-boundary.verification.boundary-rail] [covers=sdk-agent-port-boundary.verification.boundary-rail] [evidence=evidence/boundary-rails.md]
- [x] [serial] V3: Run focused agent tests, architecture rails, Cairn gates/validate, and the relevant embedded SDK acceptance slice. r[sdk-agent-port-boundary.verification] [covers=sdk-agent-port-boundary.verification] [evidence=evidence/validation-closeout.md]
