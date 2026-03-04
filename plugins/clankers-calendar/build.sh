#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
cargo build --target wasm32-unknown-unknown --release
target_dir="${CARGO_TARGET_DIR:-target}"
cp "$target_dir/wasm32-unknown-unknown/release/clankers_calendar.wasm" .
echo "Built clankers_calendar.wasm"
