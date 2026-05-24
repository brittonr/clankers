# Change: Steel eval controlled corpus dogfood

## Why

`steel_eval` is available, but promotion beyond mechanical availability needs measured dogfood over a reviewed local corpus/profile with thresholds, regression budget, and safe receipts.

## What Changes

- Define a controlled local corpus/profile contract for Steel eval dogfood.
- Require threshold/regression-budget evaluation before any promotion-eligible claim.
- Record deterministic receipts that distinguish unchanged/noise, improvements, regressions, blocked corpus, and redaction outcomes.

## Non-Goals

- No automatic promotion.
- No remote corpus fetching or credentials.
- No mutation authority or host-function expansion in the default profile.
