#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
cargo build --target wasm32-unknown-unknown --release
cp ~/.cargo-target/wasm32-unknown-unknown/release/clankers_github.wasm .
echo "Built clankers_github.wasm"
