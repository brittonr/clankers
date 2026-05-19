#!/usr/bin/env bash
set -euo pipefail
CDPATH=
export CDPATH

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null && pwd)"
readonly REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." >/dev/null && pwd)"
readonly EXAMPLE_MANIFEST="examples/embedded-agent-sdk/Cargo.toml"
readonly MINIMAL_KIT_MANIFEST="examples/embedded-minimal-kit/Cargo.toml"
readonly TOOL_KIT_MANIFEST="examples/embedded-tool-kit/Cargo.toml"
readonly PROVIDER_ADAPTER_MANIFEST="examples/embedded-provider-adapter/Cargo.toml"
readonly SESSION_STORE_MANIFEST="examples/embedded-session-store/Cargo.toml"
readonly PRODUCT_WORKBENCH_MANIFEST="examples/embedded-product-workbench/Cargo.toml"
readonly PROMPT_ASSEMBLY_KIT_MANIFEST="examples/prompt-assembly-kit/Cargo.toml"
readonly CONFIRMATION_BROKER_KIT_MANIFEST="examples/confirmation-broker-kit/Cargo.toml"
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

: "${TMPDIR:=${HOME}/.cargo-target/tmp}"
mkdir -p "${TMPDIR}"
export TMPDIR

run_step "${SCRIPT_DIR}/check-embedded-sdk-api.rs"
run_step "${SCRIPT_DIR}/check-embedded-lego-contracts.rs"
run_step "${SCRIPT_DIR}/check-real-product-dogfood.rs"
run_step "${SCRIPT_DIR}/check-provider-adapter-kit.rs"
run_step "${SCRIPT_DIR}/check-session-resume-brick.rs"
run_step "${SCRIPT_DIR}/check-tool-catalog-manifest.rs"
run_step "${SCRIPT_DIR}/check-capability-pack-composition.rs"
run_step "${SCRIPT_DIR}/check-embedded-sdk-deps.rs"
run_step "${SCRIPT_DIR}/check-embedded-adapters-deps.rs"
run_step "${SCRIPT_DIR}/check-llm-contract-boundary.sh"
run_step "${SCRIPT_DIR}/check-engine-host-feature-matrix.rs"
run_step "${SCRIPT_DIR}/check-tool-catalog-matrix.rs"
run_step "${SCRIPT_DIR}/check-runtime-extension-service-matrix.rs"
run_step "${SCRIPT_DIR}/check-shell-adapter-parity-matrix.rs"
run_step "${SCRIPT_DIR}/check-batch-eval-runner-kit.rs"
run_step "${SCRIPT_DIR}/check-slash-command-routing-kit.rs"
run_step "${SCRIPT_DIR}/check-tui-action-menu-kit.rs"
run_step "${SCRIPT_DIR}/check-daemon-event-translation-kit.rs"
run_step "${SCRIPT_DIR}/check-controller-continuation-policy-kit.rs"
run_step "${SCRIPT_DIR}/check-observability-audit-receipt-kit.rs"
run_step "${SCRIPT_DIR}/check-self-evolution-receipt-chain-kit.rs"
run_step "${SCRIPT_DIR}/check-process-job-profile-kit.rs"
run_step "${SCRIPT_DIR}/emit-embedded-sdk-release-receipt.rs"
run_cargo_step run --locked --manifest-path "${EXAMPLE_MANIFEST}"
run_cargo_step run --locked --manifest-path "${MINIMAL_KIT_MANIFEST}"
run_cargo_step run --locked --manifest-path "${TOOL_KIT_MANIFEST}"
run_cargo_step run --locked --manifest-path "${PROVIDER_ADAPTER_MANIFEST}"
run_cargo_step run --locked --manifest-path "${SESSION_STORE_MANIFEST}"
run_cargo_step run --locked --manifest-path "${PRODUCT_WORKBENCH_MANIFEST}"
run_cargo_step run --locked --manifest-path "${PROMPT_ASSEMBLY_KIT_MANIFEST}"
run_cargo_step run --locked --manifest-path "${CONFIRMATION_BROKER_KIT_MANIFEST}"
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
