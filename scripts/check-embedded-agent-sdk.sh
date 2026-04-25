#!/usr/bin/env bash
set -euo pipefail

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
readonly EXAMPLE_MANIFEST="examples/embedded-agent-sdk/Cargo.toml"
readonly AGENT_TURN_TEST_FILTER="turn::tests::"

run_step() {
  printf '\n==> %s\n' "$*"
  "$@"
}

run_cargo_step() {
  printf '\n==> cargo %s\n' "$*"
  RUSTC_WRAPPER= cargo "$@"
}

cd "${REPO_ROOT}"

run_step "${SCRIPT_DIR}/check-embedded-sdk-api.rs"
run_step "${SCRIPT_DIR}/check-embedded-sdk-deps.rs"
run_step "${SCRIPT_DIR}/check-llm-contract-boundary.sh"
run_cargo_step run --locked --manifest-path "${EXAMPLE_MANIFEST}"
run_cargo_step test -p clankers-engine-host --lib
run_cargo_step test -p clankers-agent --lib "${AGENT_TURN_TEST_FILTER}"
run_cargo_step test -p clankers-controller --test fcis_shell_boundaries

printf '\nembedded-agent-sdk acceptance passed\n'
