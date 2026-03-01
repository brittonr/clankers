#!/usr/bin/env bash
# Launch the E2E test environment in Zellij
# Usage: ./tests/e2e/zellij.sh
set -euo pipefail

cd "$(dirname "$0")/../.."
LAYOUT="$(pwd)/tests/e2e/layout.kdl"
SESSION="clankers-e2e"

# Kill existing session if present
zellij delete-session "$SESSION" --force 2>/dev/null || true

echo "Launching Zellij E2E environment..."
echo ""
echo "  Tab 1 (tests):       test runner + log viewer"
echo "  Tab 2 (interactive): launch clankers TUI manually"
echo "  Tab 3 (print):       manual print/tool testing"
echo ""
echo "In the runner pane, run:"
echo "  cd $(pwd) && nix develop --command bash tests/e2e/run-tests.sh"
echo ""

exec zellij --session "$SESSION" --layout "$LAYOUT"
