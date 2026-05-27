# Lifecycle Specification

## Requirements

### Requirement: Review notes are collected
The lifecycle package MUST include review notes somewhere in the change.

#### Scenario: generated artifacts may be summarized
- GIVEN generated artifacts changed
- WHEN reviewers inspect evidence
- THEN generated artifact hygiene may be mentioned when available

#### Scenario: local verification can be summarized
- GIVEN tests were run locally
- WHEN reviewers inspect evidence
- THEN local verification and contract coverage should be summarized as optional evidence
