#!/usr/bin/env bash
set -euo pipefail

readonly TARGET_TRIPLE="thumbv7em-none-eabi"
readonly BUILD_STD_COMPONENTS="core,alloc"
readonly CORE_PACKAGE="clankers-core"

cargo check \
  -Zbuild-std="${BUILD_STD_COMPONENTS}" \
  -p "${CORE_PACKAGE}" \
  --no-default-features \
  --target "${TARGET_TRIPLE}"
