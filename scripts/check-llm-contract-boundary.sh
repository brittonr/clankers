#!/usr/bin/env bash
set -euo pipefail

ENGINE_PACKAGE="clankers-engine"
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


ENGINE_FORBIDDEN_SOURCE_TOKENS=(
  "core_state"
  "CoreState"
  "CoreEffectId"
  "clankers_core"
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
