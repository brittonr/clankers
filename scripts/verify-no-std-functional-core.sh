#!/usr/bin/env bash
set -euo pipefail

run_step() {
  local label="$1"
  shift

  echo "=== ${label} ==="
  "$@"
  echo "  ✓ ${label}"
  echo
}

run_step "No-std core bare-metal compile rail" ./scripts/check-clankers-core-nostd.sh
run_step "No-std core dependency and API boundary rail" ./scripts/check-clankers-core-boundary.sh
run_step "No-std core exported-surface rail" ./scripts/check-clankers-core-surface.sh
run_step "No-std core reducer suite" cargo test -p clankers-core --lib
run_step "No-std core determinism rail" cargo test -p clankers-core --test determinism
run_step "Controller reducer and parity suites" cargo nextest run -p clankers-controller --tests
run_step "Agent adapter parity suite" cargo test -p clankers-agent user_tool_filter --lib
run_step "Embedded controller parity suite" cargo nextest run --test embedded_controller

echo "=== No-std functional core validation bundle passed ==="
