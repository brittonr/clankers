#!/usr/bin/env bash
set -euo pipefail
CDPATH=
export CDPATH

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null && pwd)"
readonly REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." >/dev/null && pwd)"
cd "${REPO_ROOT}"
exec "${SCRIPT_DIR}/check-embedded-agent-sdk.rs" "$@"
