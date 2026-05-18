#!/usr/bin/env bash
set -euo pipefail
CDPATH=
export CDPATH

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null && pwd)"
readonly REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." >/dev/null && pwd)"
readonly EXAMPLE_MANIFEST="examples/embedded-agent-sdk/Cargo.toml"
readonly MINIMAL_KIT_MANIFEST="examples/embedded-minimal-kit/Cargo.toml"
readonly TOOL_KIT_MANIFEST="examples/embedded-tool-kit/Cargo.toml"
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
run_step "${SCRIPT_DIR}/check-embedded-adapters-deps.rs"
run_step "${SCRIPT_DIR}/check-llm-contract-boundary.sh"
run_step "${SCRIPT_DIR}/check-engine-host-feature-matrix.rs"
run_step "${SCRIPT_DIR}/check-tool-catalog-matrix.rs"
run_step "${SCRIPT_DIR}/check-runtime-extension-service-matrix.rs"
run_step "${SCRIPT_DIR}/check-shell-adapter-parity-matrix.rs"
run_cargo_step run --locked --manifest-path "${EXAMPLE_MANIFEST}"
run_cargo_step run --locked --manifest-path "${MINIMAL_KIT_MANIFEST}"
run_cargo_step run --locked --manifest-path "${TOOL_KIT_MANIFEST}"
run_cargo_step test -p clankers-adapters --lib
run_cargo_step test -p clankers-adapters --lib replaceable
run_cargo_step test -p clankers-adapters --lib tool_catalog_metadata
run_cargo_step test -p clankers-adapters --lib tool_catalog_validation
run_cargo_step test -p clankers-adapters --lib capability_pack
run_cargo_step test -p clankers-engine-host --lib
run_cargo_step test -p clankers-agent --lib "${AGENT_TURN_TEST_FILTER}"
run_cargo_step test -p clankers-runtime --lib tool_catalog_
run_cargo_step test -p clankers-runtime --lib runtime_extension_service_matrix_
run_cargo_step test -p clankers-controller --test fcis_shell_boundaries

printf '\nembedded-agent-sdk acceptance passed\n'
