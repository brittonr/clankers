#!/usr/bin/env bash
set -euo pipefail

readonly CORE_MANIFEST="crates/clankers-core/Cargo.toml"
readonly CORE_SOURCE_DIR="crates/clankers-core/src"
readonly NOT_FOUND_STATUS="1"

readonly MANIFEST_PATTERNS=(
  "tokio"
  "tokio-util"
  "crossterm"
  "ratatui"
  "redb"
  "reqwest"
  "iroh"
  "portable-pty"
)

readonly SOURCE_PATTERNS=(
  "portable_pty::"
  "std::env"
  "std::fs"
  "std::net"
  "std::process"
  "std::time"
  "chrono::"
  "crossterm::"
  "iroh::"
  "ratatui::"
  "redb::"
  "reqwest::"
  "tokio::"
)

report_matches() {
  local -n patterns_ref="$1"
  local search_root="$2"
  local category_label="$3"
  local pattern
  local matches_status

  for pattern in "${patterns_ref[@]}"; do
    set +e
    rg --line-number --fixed-strings "${pattern}" "${search_root}"
    matches_status=$?
    set -e

    if [[ "${matches_status}" == "${NOT_FOUND_STATUS}" ]]; then
      continue
    fi

    if [[ "${matches_status}" != "0" ]]; then
      echo "boundary check failed while scanning ${category_label} for pattern: ${pattern}" >&2
      exit "${matches_status}"
    fi

    echo "forbidden ${category_label} pattern found: ${pattern}" >&2
    exit 1
  done
}

report_matches MANIFEST_PATTERNS "${CORE_MANIFEST}" "manifest"
report_matches SOURCE_PATTERNS "${CORE_SOURCE_DIR}" "source"
