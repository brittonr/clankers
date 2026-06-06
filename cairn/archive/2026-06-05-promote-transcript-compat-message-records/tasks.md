## Phase 1: Implementation

- [x] [serial] I1: Promote `UserMessage`, `AssistantMessage`, `ToolResultMessage`, and their fields from experimental to optional support in the embedded SDK inventory. r[embedded-composition-kits.experimental-port-budget.transcript-compat-records-supported] [covers=embedded-composition-kits.experimental-port-budget.transcript-compat-records-supported] [evidence=evidence/transcript-compat-message-records.md]
- [x] [serial] I2: Update the experimental-budget rail and policy so promoted optional-support groups drain the remaining experimental rows to zero. r[embedded-composition-kits.experimental-port-budget.transcript-compat-records-supported] [covers=embedded-composition-kits.experimental-port-budget.transcript-compat-records-supported] [evidence=evidence/transcript-compat-message-records.md]
- [x] [serial] I3: Refresh brick stability artifacts after the transcript compatibility stability change. r[embedded-composition-kits.experimental-port-budget.transcript-compat-records-supported] [covers=embedded-composition-kits.experimental-port-budget.transcript-compat-records-supported] [evidence=evidence/transcript-compat-message-records.md]

## Phase 2: Verification

- [x] [serial] V1: Run focused transcript compatibility tests plus message/inventory/budget/brick rails. r[embedded-composition-kits.experimental-port-budget.transcript-compat-records-supported] [covers=embedded-composition-kits.experimental-port-budget.transcript-compat-records-supported] [evidence=evidence/transcript-compat-message-records.md]
- [x] [serial] V2: Run aggregate embedded SDK acceptance plus Cairn validation/gates and `git diff --check`. r[embedded-composition-kits.experimental-port-budget.transcript-compat-records-supported] [covers=embedded-composition-kits.experimental-port-budget.transcript-compat-records-supported] [evidence=evidence/validation-closeout.md]
