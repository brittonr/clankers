#!/usr/bin/env bash
set -euo pipefail

ENGINE_PACKAGE="clankers-engine"
ENGINE_HOST_PACKAGE="clankers-engine-host"
TOOL_HOST_PACKAGE="clankers-tool-host"
MESSAGE_PACKAGE="clanker-message"
CARGO_TREE_EDGES="normal"

ENGINE_FORBIDDEN_CRATES=(
  "clankers-core"
  "clankers-provider"
  "clanker-router"
  "tokio"
  "reqwest"
  "redb"
  "iroh"
  "ratatui"
  "crossterm"
  "portable-pty"
  "clankers-agent"
)


HOST_FORBIDDEN_DIRECT_CRATES=(
  "clankers-agent"
  "clankers-core"
  "clankers-controller"
  "clankers-provider"
  "clanker-router"
  "clankers-db"
  "clankers-hooks"
  "clankers-plugin"
  "clankers-protocol"
  "clanker-tui-types"
  "clankers-tui"
  "ratatui"
  "crossterm"
  "portable-pty"
  "iroh"
  "redb"
  "reqwest"
  "hyper"
  "h2"
  "tower"
  "axum"
  "tokio"
  "async-std"
  "smol"
  "actix-rt"
  "reqwest-eventsource"
  "eventsource-stream"
  "chrono"
  "time"
  "uuid"
  "ulid"
  "clankers-config"
  "clankers-model-selection"
)

HOST_FORBIDDEN_TRANSITIVE_CRATES=(
  "clankers-agent"
  "clankers-core"
  "clankers-controller"
  "clankers-provider"
  "clanker-router"
  "clankers-db"
  "clankers-hooks"
  "clankers-plugin"
  "clankers-protocol"
  "clanker-tui-types"
  "clankers-tui"
  "ratatui"
  "crossterm"
  "portable-pty"
  "iroh"
  "redb"
  "reqwest"
  "hyper"
  "h2"
  "tower"
  "axum"
  "tokio"
  "async-std"
  "smol"
  "actix-rt"
  "reqwest-eventsource"
  "eventsource-stream"
  "clankers-config"
  "clankers-model-selection"
)

MESSAGE_FORBIDDEN_CRATES=(
  "clanker-router"
  "clankers-provider"
  "tokio"
  "reqwest"
  "reqwest-eventsource"
  "redb"
  "fs4"
  "iroh"
  "axum"
  "tower-http"
  "ratatui"
  "crossterm"
  "portable-pty"
)

crate_pattern() {
  local crate_name="$1"
  printf '%s v' "$crate_name"
}


check_direct_normal_deps_exclude() {
  local package_name="$1"
  shift
  local forbidden_csv
  forbidden_csv=$(IFS=,; printf '%s' "$*")

  local metadata
  metadata=$(cargo metadata --format-version 1 --no-deps)

  local found
  found=$(PACKAGE_NAME="$package_name" FORBIDDEN_CRATES="$forbidden_csv" python3 -c '
import json
import os
import sys

metadata = json.load(sys.stdin)
package_name = os.environ["PACKAGE_NAME"]
forbidden = set(filter(None, os.environ["FORBIDDEN_CRATES"].split(",")))
package = next((p for p in metadata["packages"] if p["name"] == package_name), None)
if package is None:
    print(f"package not found in cargo metadata: {package_name}", file=sys.stderr)
    sys.exit(2)
for dep in package.get("dependencies", []):
    if dep.get("kind") not in (None, "normal"):
        continue
    name = dep.get("rename") or dep["name"]
    package = dep.get("package") or dep["name"]
    if name in forbidden or package in forbidden:
        print(package)
' <<<"$metadata" | sort -u)

  if [[ -n "$found" ]]; then
    printf 'forbidden direct normal dependency in %s: %s\n' "$package_name" "$found" >&2
    return 1
  fi

  printf 'ok: %s direct normal deps exclude forbidden crates\n' "$package_name"
}

check_tree_excludes() {
  local package_name="$1"
  shift
  local tree_output
  tree_output=$(cargo tree -p "$package_name" --edges "$CARGO_TREE_EDGES")

  local found=0
  local crate_name
  for crate_name in "$@"; do
    if grep -Fq "$(crate_pattern "$crate_name")" <<<"$tree_output"; then
      printf 'forbidden dependency in %s normal-edge tree: %s\n' "$package_name" "$crate_name" >&2
      found=1
    fi
  done

  if [[ "$found" -ne 0 ]]; then
    printf '\n--- cargo tree -p %s --edges %s ---\n%s\n' "$package_name" "$CARGO_TREE_EDGES" "$tree_output" >&2
    return 1
  fi

  printf 'ok: %s normal-edge tree excludes forbidden crates\n' "$package_name"
}

check_tree_excludes "$ENGINE_PACKAGE" "${ENGINE_FORBIDDEN_CRATES[@]}"
check_tree_excludes "$ENGINE_HOST_PACKAGE" "${HOST_FORBIDDEN_TRANSITIVE_CRATES[@]}"
check_tree_excludes "$TOOL_HOST_PACKAGE" "${HOST_FORBIDDEN_TRANSITIVE_CRATES[@]}"
check_tree_excludes "$MESSAGE_PACKAGE" "${MESSAGE_FORBIDDEN_CRATES[@]}"
check_direct_normal_deps_exclude "$ENGINE_HOST_PACKAGE" "${HOST_FORBIDDEN_DIRECT_CRATES[@]}"
check_direct_normal_deps_exclude "$TOOL_HOST_PACKAGE" "${HOST_FORBIDDEN_DIRECT_CRATES[@]}"


ENGINE_FORBIDDEN_SOURCE_TOKENS=(
  "core_state"
  "CoreState"
  "CoreEffectId"
  "clankers_core"
  "clankers_provider"
  "clanker_router"
  "clankers_protocol"
  "clanker_tui_types"
  "clankers_db"
  "CompletionRequest"
  "CompletionResponse"
  "ProviderResponse"
  "tokio::runtime::Handle"
  "tokio::task::JoinHandle"
  "reqwest::Client"
  "AgentMessage"
  "MessageId"
  "Utc"
  "DateTime"
  "Instant::now"
  "SystemTime"
  "OnceLock"
  "OnceCell"
  "LazyLock"
  "lazy_static"
  "service_locator"
  "global_service"
  "singleton"
)

HOST_FORBIDDEN_SOURCE_TOKENS=(
  "clankers_agent"
  "clankers_provider"
  "clanker_router"
  "clankers_protocol"
  "clanker_tui_types"
  "clankers_db"
  "clankers_config"
  "CompletionRequest"
  "CompletionResponse"
  "ProviderResponse"
  "tokio::runtime::Handle"
  "tokio::task::JoinHandle"
  "reqwest::Client"
  "AgentMessage"
  "MessageId"
  "Utc"
  "DateTime"
  "Instant::now"
  "SystemTime"
  "OnceLock"
  "OnceCell"
  "LazyLock"
  "lazy_static"
  "service_locator"
  "global_service"
  "singleton"
)

TOOL_HOST_FORBIDDEN_SOURCE_TOKENS=(
  "clankers_agent"
  "clankers_provider"
  "clanker_router"
  "clankers_protocol"
  "clanker_tui_types"
  "clankers_db"
  "clankers_config"
  "CompletionRequest"
  "CompletionResponse"
  "ProviderResponse"
  "tokio::runtime::Handle"
  "tokio::task::JoinHandle"
  "reqwest::Client"
  "AgentMessage"
  "MessageId"
  "Utc"
  "DateTime"
  "Instant::now"
  "SystemTime"
  "OnceLock"
  "OnceCell"
  "LazyLock"
  "lazy_static"
  "service_locator"
  "global_service"
  "singleton"
)

check_source_excludes_tokens() {
  local relative_dir="$1"
  shift
  local found=0
  local token
  local file

  while IFS= read -r -d '' file; do
    for token in "$@"; do
      if grep -nF -- "$token" "$file" >/tmp/clankers-boundary-grep.$$; then
        while IFS= read -r match; do
          printf 'forbidden source token in %s: %s: %s
' "$file" "$token" "$match" >&2
        done </tmp/clankers-boundary-grep.$$
        found=1
      fi
    done
  done < <(find "$relative_dir" -type f -name '*.rs' -print0 | sort -z)
  rm -f /tmp/clankers-boundary-grep.$$

  if [[ "$found" -ne 0 ]]; then
    return 1
  fi

  printf 'ok: %s excludes forbidden source tokens
' "$relative_dir"
}

check_source_excludes_tokens "crates/clankers-engine/src" "${ENGINE_FORBIDDEN_SOURCE_TOKENS[@]}"
check_source_excludes_tokens "crates/clankers-engine-host/src" "${HOST_FORBIDDEN_SOURCE_TOKENS[@]}"
check_source_excludes_tokens "crates/clankers-tool-host/src" "${TOOL_HOST_FORBIDDEN_SOURCE_TOKENS[@]}"
