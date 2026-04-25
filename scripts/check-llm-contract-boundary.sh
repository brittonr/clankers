#!/usr/bin/env bash
set -euo pipefail

ENGINE_PACKAGE="clankers-engine"
MESSAGE_PACKAGE="clanker-message"
CARGO_TREE_EDGES="normal"

ENGINE_FORBIDDEN_CRATES=(
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
check_tree_excludes "$MESSAGE_PACKAGE" "${MESSAGE_FORBIDDEN_CRATES[@]}"
