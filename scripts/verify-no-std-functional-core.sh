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
run_step "Agent turn allowlist parity suite" cargo test -p clankers-agent user_tool_filter --lib
run_step "Agent filtered inventory parity suite" cargo test -p clankers-agent controller_filtered_tool_inventory_replaces_available_tools_without_turn_local_state --lib
run_step "Agent thinking adapter parity suite" cargo test -p clankers-agent agent_applies_core_thinking_effect_without_agent_owned_reducer --lib
run_step "Agent tool inventory adapter parity suite" cargo test -p clankers-agent agent_tool_inventory_can_follow_core_disabled_tool_contract_without_local_policy --lib
run_step "Embedded controller parity suite" cargo nextest run --test embedded_controller

echo "=== No-std functional core validation bundle passed ==="
