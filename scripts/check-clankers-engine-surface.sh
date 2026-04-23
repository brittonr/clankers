#!/usr/bin/env bash
set -euo pipefail

readonly ENGINE_SURFACE_FILES=(
  "crates/clankers-engine/src/lib.rs"
)
readonly NOT_FOUND_STATUS="1"

readonly SURFACE_PATTERNS=(
  "clankers_agent::"
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
  "tokio_util::"
)
readonly TEST_MODULE_PATTERN='^#\[cfg\(test\)\]'

surface_source() {
  python - <<'PY'
from pathlib import Path
path = Path('crates/clankers-engine/src/lib.rs')
text = path.read_text()
marker = '#[cfg(test)]\nmod tests {'
index = text.find(marker)
if index == -1:
    print(text, end='')
else:
    print(text[:index], end='')
PY
}

for pattern in "${SURFACE_PATTERNS[@]}"; do
  set +e
  surface_source | rg --line-number --fixed-strings "${pattern}"
  matches_status=$?
  set -e

  if [[ "${matches_status}" == "${NOT_FOUND_STATUS}" ]]; then
    continue
  fi

  if [[ "${matches_status}" != "0" ]]; then
    echo "surface check failed while scanning exported engine boundary for pattern: ${pattern}" >&2
    exit "${matches_status}"
  fi

  echo "forbidden exported engine boundary pattern found: ${pattern}" >&2
  exit 1
done
