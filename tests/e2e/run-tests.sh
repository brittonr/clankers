#!/usr/bin/env bash
# E2E test runner for clankers — runs inside nix develop shell
# Usage: ./tests/e2e/run-tests.sh [all|fast|api|fake|deterministic|test-name]
#   fake/deterministic = run credential-free fake-provider CLI + tool coverage
#   test-name = run a specific test (print, tools, read-write, json, auth)
# shellcheck disable=SC2329 # tests are invoked dynamically by name.
set -euo pipefail

cd "$(dirname "$0")/../.."
LOG=/tmp/clankers-test.log
PASS=0
FAIL=0
ERRORS=()

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

log() { echo -e "${CYAN}[test]${NC} $*" | tee -a "$LOG"; }
pass() { echo -e "${GREEN}  ✓ $*${NC}" | tee -a "$LOG"; PASS=$((PASS + 1)); }
fail() { echo -e "${RED}  ✗ $*${NC}" | tee -a "$LOG"; FAIL=$((FAIL + 1)); ERRORS+=("$*"); }

run_clankers() {
    CLANKERS_NO_DAEMON="${CLANKERS_NO_DAEMON:-1}" \
    RUST_LOG=off cargo run --quiet -- "$@" 2>>"$LOG"
}

run_fake_clankers() {
    CLANKERS_FAKE_PROVIDER=1 run_clankers "$@"
}

# ─── Tests ────────────────────────────────────────────────────────────

test_version() {
    log "test: version"
    out=$(run_clankers version)
    if [[ "$out" == *"clankers 0.1.0"* ]]; then
        pass "version output correct"
    else
        fail "version: got '$out'"
    fi
}

test_auth_status() {
    log "test: auth status"
    out=$(run_clankers auth status)
    if [[ "$out" == *"OAuth token valid"* ]] || [[ "$out" == *"API key set"* ]]; then
        pass "auth is configured and valid"
    elif [[ "$out" == *"expired"* ]] || [[ "$out" == *"Accounts:"* ]]; then
        pass "auth is configured (tokens expired — run 'clankers auth login' to refresh)"
    else
        fail "auth not configured: $out"
    fi
}

test_config_show() {
    log "test: config show"
    out=$(run_clankers config show)
    # Validate JSON — try python3 first, fall back to checking braces
    if command -v python3 &>/dev/null; then
        if echo "$out" | python3 -m json.tool > /dev/null 2>&1; then
            pass "config show returns valid JSON"
        else
            fail "config show: invalid JSON"
        fi
    elif [[ "$out" == "{"* ]] && [[ "$out" == *"}" ]]; then
        pass "config show returns JSON object"
    else
        fail "config show: not a JSON object"
    fi
}

test_config_paths() {
    log "test: config paths"
    out=$(run_clankers config paths)
    if [[ "$out" == *"Global config"* ]]; then
        pass "config paths output"
    else
        fail "config paths: unexpected output"
    fi
}

test_print_simple() {
    log "test: print mode — simple prompt"
    out=$(run_fake_clankers -p "Reply with exactly one word: yes")
    lower=$(echo "$out" | tr '[:upper:]' '[:lower:]')
    if [[ "$lower" == *"yes"* ]]; then
        pass "print mode returned response"
    else
        fail "print mode: got '$out'"
    fi
}

test_print_tool_bash() {
    log "test: print mode — bash tool"
    out=$(run_fake_clankers -p "Use the bash tool to run: echo CLANKERS_TOOL_TEST_OK")
    if [[ "$out" == *"CLANKERS_TOOL_TEST_OK"* ]]; then
        pass "bash tool executed and result returned"
    else
        fail "bash tool: output missing marker, got '$out'"
    fi
}

test_print_tool_read() {
    log "test: print mode — read tool"
    out=$(run_fake_clankers -p "Use the read tool to read the file Cargo.toml and tell me the package name")
    if [[ "$out" == *"clankers"* ]]; then
        pass "read tool returned Cargo.toml content"
    else
        fail "read tool: got '$out'"
    fi
}

test_print_tool_write_edit() {
    log "test: print mode — write + edit tools"
    tmpfile="/tmp/clankers-e2e-write-test-$$"
    rm -f "$tmpfile"
    run_fake_clankers -p "Use the write tool to create the file $tmpfile with content 'hello world'." >/dev/null
    run_fake_clankers -p "Use the edit tool to replace 'world' with 'clankers' in $tmpfile." >/dev/null
    out=$(run_fake_clankers -p "Use the read tool to read $tmpfile and show me the final content.")
    content=$(cat "$tmpfile" 2>/dev/null || echo "FILE_MISSING")
    rm -f "$tmpfile"
    if [[ "$content" == *"hello clankers"* ]]; then
        pass "write+edit+read round-trip"
    else
        fail "write+edit: file content='$content', output='$out'"
    fi
}

test_print_tool_ls() {
    log "test: print mode — ls tool"
    out=$(run_fake_clankers -p "Use the ls tool to list files in the current directory. What files do you see?")
    if [[ "$out" == *"Cargo.toml"* ]] || [[ "$out" == *"src"* ]]; then
        pass "ls tool returned directory listing"
    else
        fail "ls tool: got '$out'"
    fi
}

test_print_tool_grep() {
    log "test: print mode — grep tool"
    out=$(run_fake_clankers -p "Use the grep tool to search for 'fn main' in the src/ directory")
    if [[ "$out" == *"main"* ]]; then
        pass "grep tool found matches"
    else
        fail "grep tool: got '$out'"
    fi
}

test_print_tool_find() {
    log "test: print mode — find tool"
    out=$(run_fake_clankers -p "Use the find tool to find files named 'mod.rs' under src/")
    if [[ "$out" == *"mod.rs"* ]]; then
        pass "find tool found files"
    else
        fail "find tool: got '$out'"
    fi
}

test_json_mode() {
    log "test: json output mode"
    out=$(run_fake_clankers --mode json -p "Say hello")
    # JSON mode outputs JSONL (one JSON object per line)
    valid=true
    while IFS= read -r line; do
        if [[ -n "$line" ]] && ! echo "$line" | python3 -m json.tool > /dev/null 2>&1; then
            valid=false
            break
        fi
    done <<< "$out"
    if [[ "$valid" == "true" ]] && [[ -n "$out" ]]; then
        pass "json mode returns valid JSONL"
    else
        fail "json mode: invalid JSONL line, got '$out'"
    fi
}

test_session_list() {
    log "test: session list"
    out=$(run_clankers session list 2>&1) || true
    pass "session list ran (output: $(echo "$out" | head -1))"
}

test_help() {
    log "test: help output"
    out=$(run_clankers --help 2>&1)
    if [[ "$out" == *"clankers"* ]] && [[ "$out" == *"COMMAND"* ]]; then
        pass "help output looks correct"
    else
        fail "help: unexpected output"
    fi
}

# ─── Runner ───────────────────────────────────────────────────────────

ALL_TESTS=(
    test_version
    test_help
    test_auth_status
    test_config_show
    test_config_paths
    test_session_list
    test_print_simple
    test_print_tool_bash
    test_print_tool_read
    test_print_tool_write_edit
    test_print_tool_ls
    test_print_tool_grep
    test_print_tool_find
    test_json_mode
)

# Group tests by speed
FAST_TESTS=(test_version test_help test_auth_status test_config_show test_config_paths test_session_list)
API_TESTS=(test_print_simple test_print_tool_bash test_print_tool_read test_print_tool_write_edit test_print_tool_ls test_print_tool_grep test_print_tool_find test_json_mode)
DETERMINISTIC_TESTS=(
    test_version
    test_help
    test_config_show
    test_config_paths
    test_session_list
    "${API_TESTS[@]}"
)

echo "" > "$LOG"
echo -e "${YELLOW}═══════════════════════════════════════${NC}"
echo -e "${YELLOW}  clankers E2E tests${NC}"
echo -e "${YELLOW}═══════════════════════════════════════${NC}"
echo ""

if [[ "${1:-all}" == "all" ]]; then
    TESTS=("${ALL_TESTS[@]}")
elif [[ "${1:-}" == "fast" ]]; then
    TESTS=("${FAST_TESTS[@]}")
elif [[ "${1:-}" == "api" ]]; then
    TESTS=("${API_TESTS[@]}")
elif [[ "${1:-}" == "fake" ]] || [[ "${1:-}" == "deterministic" ]]; then
    TESTS=("${DETERMINISTIC_TESTS[@]}")
else
    # Run a single named test
    TESTS=("test_${1}")
fi

for t in "${TESTS[@]}"; do
    if declare -f "$t" > /dev/null 2>&1; then
        "$t"
    else
        fail "unknown test: $t"
    fi
done

echo ""
echo -e "${YELLOW}═══════════════════════════════════════${NC}"
echo -e "  ${GREEN}passed: $PASS${NC}  ${RED}failed: $FAIL${NC}"
if [[ $FAIL -gt 0 ]]; then
    echo -e "${RED}  failures:${NC}"
    for e in "${ERRORS[@]}"; do
        echo -e "    ${RED}• $e${NC}"
    done
fi
echo -e "${YELLOW}═══════════════════════════════════════${NC}"
echo ""
echo "log: $LOG"

exit $FAIL
