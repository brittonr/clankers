#!/usr/bin/env bash
set -euo pipefail

readonly CORE_SURFACE_FILES=(
  "crates/clankers-core/src/lib.rs"
  "crates/clankers-core/src/types.rs"
)
readonly NOT_FOUND_STATUS="1"

readonly SURFACE_PATTERNS=(
  "clankers_agent::events::AgentEvent"
  "clanker_message"
  "clankers_protocol::DaemonEvent"
  "clankers_protocol::SessionCommand"
  "chrono::"
  "crossterm::"
  "iroh::"
  "portable_pty::"
  "ratatui::"
  "redb::"
  "reqwest::"
  "tokio::"
)

for pattern in "${SURFACE_PATTERNS[@]}"; do
  set +e
  rg --line-number --fixed-strings "${pattern}" "${CORE_SURFACE_FILES[@]}"
  matches_status=$?
  set -e

  if [[ "${matches_status}" == "${NOT_FOUND_STATUS}" ]]; then
    continue
  fi

  if [[ "${matches_status}" != "0" ]]; then
    echo "surface check failed while scanning exported core boundary for pattern: ${pattern}" >&2
    exit "${matches_status}"
  fi

  echo "forbidden exported core boundary pattern found: ${pattern}" >&2
  exit 1
done
