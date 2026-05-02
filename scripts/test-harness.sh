#!/usr/bin/env bash
# Canonical local test harness for clankers.
#
# Usage:
#   ./scripts/test-harness.sh quick
#   ./scripts/test-harness.sh package <crate> [filter...]
#   ./scripts/test-harness.sh full
#   ./scripts/test-harness.sh e2e [fast|api|all|test-name]
#   ./scripts/test-harness.sh vm [all|core|module|smoke|check-name]
#   ./scripts/test-harness.sh ci [extra nix args...]
#
# Set CLANKERS_TEST_DRY_RUN=1 to print the selected commands without running them.
set -euo pipefail

cd "$(dirname "$0")/.."

MODE="${1:-quick}"
if [[ $# -gt 0 ]]; then
    shift
fi

RESULT_DIR="${CLANKERS_TEST_RESULT_DIR:-target/test-harness}"
SUMMARY_MD="$RESULT_DIR/summary.md"
RESULTS_JSON="$RESULT_DIR/results.json"
JUNIT_XML="$RESULT_DIR/junit.xml"
LOG_DIR="$RESULT_DIR/logs"
DRY_RUN="${CLANKERS_TEST_DRY_RUN:-0}"

mkdir -p "$LOG_DIR"

STARTED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0
RESULT_ITEMS=()
RESULT_NAMES=()
RESULT_STATUSES=()
RESULT_EXIT_CODES=()
RESULT_LOGS=()
RESULT_COMMANDS=()

# Keep cargo output and runtime state predictable for agent/CI runs.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target}"
export CLANKERS_NO_DAEMON="${CLANKERS_NO_DAEMON:-1}"

usage() {
    awk '
        NR == 1 { next }
        /^# ?/ {
            line = $0
            sub(/^# ?/, "", line)
            print line
            next
        }
        { exit }
    ' "$0"
}

json_escape() {
    python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))'
}

xml_escape() {
    local value="$1"
    value="${value//&/&amp;}"
    value="${value//</&lt;}"
    value="${value//>/&gt;}"
    value="${value//\"/&quot;}"
    value="${value//\'/&apos;}"
    printf '%s' "$value"
}

append_result() {
    local name="$1"
    local status="$2"
    local exit_code="$3"
    local log_file="$4"
    local command_text="$5"
    local command_json log_json
    command_json="$(printf '%s' "$command_text" | json_escape)"
    log_json="$(printf '%s' "$log_file" | json_escape)"
    RESULT_ITEMS+=("{\"name\":\"$name\",\"status\":\"$status\",\"exit_code\":$exit_code,\"command\":$command_json,\"log\":$log_json}")
    RESULT_NAMES+=("$name")
    RESULT_STATUSES+=("$status")
    RESULT_EXIT_CODES+=("$exit_code")
    RESULT_LOGS+=("$log_file")
    RESULT_COMMANDS+=("$command_text")
}

run_step() {
    local name="$1"
    shift
    local log_file="$LOG_DIR/${name//[^A-Za-z0-9_.-]/_}.log"
    local command_text="$*"

    echo "==> $name"
    echo "    $command_text"

    if [[ "$DRY_RUN" == "1" ]]; then
        printf 'DRY RUN: %s\n' "$command_text" > "$log_file"
        append_result "$name" "skipped" 0 "$log_file" "$command_text"
        SKIP_COUNT=$((SKIP_COUNT + 1))
        return 0
    fi

    set +e
    "$@" > >(tee "$log_file") 2>&1
    local exit_code=$?
    set -e

    if [[ $exit_code -eq 0 ]]; then
        append_result "$name" "passed" "$exit_code" "$log_file" "$command_text"
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        append_result "$name" "failed" "$exit_code" "$log_file" "$command_text"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        return "$exit_code"
    fi
}

run_shell_step() {
    local name="$1"
    shift
    run_step "$name" bash -lc "$*"
}

run_vm_check() {
    local check="$1"
    local system="$2"
    local label="${check#vm-}"
    run_step "nix vm $label" nix build ".#checks.$system.$check" --no-link -L
}

run_vm_selector() {
    local selector="${1:-all}"
    local system
    local checks=()

    system="$(nix eval --raw --impure --expr 'builtins.currentSystem')"

    case "$selector" in
        all)
            checks=(
                vm-smoke
                vm-remote-daemon
                vm-session-recovery
                vm-module-daemon
                vm-module-router
                vm-module-integration
            )
            ;;
        core)
            checks=(
                vm-smoke
                vm-remote-daemon
                vm-session-recovery
            )
            ;;
        module)
            checks=(
                vm-module-daemon
                vm-module-router
                vm-module-integration
            )
            ;;
        smoke)
            checks=(vm-smoke)
            ;;
        vm-*)
            checks=("$selector")
            ;;
        *)
            echo "error: unknown vm selector/check: $selector" >&2
            echo "known vm selectors: all, core, module, smoke" >&2
            echo "known vm checks: vm-smoke, vm-remote-daemon, vm-session-recovery, vm-module-daemon, vm-module-router, vm-module-integration" >&2
            return 2
            ;;
    esac

    local check
    for check in "${checks[@]}"; do
        run_vm_check "$check" "$system"
    done
}

write_reports() {
    local finished_at items_json
    finished_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    if [[ ${#RESULT_ITEMS[@]} -eq 0 ]]; then
        items_json="[]"
    else
        items_json="[$(IFS=,; echo "${RESULT_ITEMS[*]}")]"
    fi

    cat > "$RESULTS_JSON" <<JSON
{
  "mode": "$MODE",
  "started_at": "$STARTED_AT",
  "finished_at": "$finished_at",
  "passed": $PASS_COUNT,
  "failed": $FAIL_COUNT,
  "skipped": $SKIP_COUNT,
  "steps": $items_json
}
JSON

    {
        echo "# clankers test harness summary"
        echo
        echo "- mode: \`$MODE\`"
        echo "- started: \`$STARTED_AT\`"
        echo "- finished: \`$finished_at\`"
        echo "- passed: $PASS_COUNT"
        echo "- failed: $FAIL_COUNT"
        echo "- skipped: $SKIP_COUNT"
        echo
        echo "## Steps"
        for item in "${RESULT_ITEMS[@]}"; do
            ITEM_JSON="$item" python3 - <<'PY'
import json
import os

item = json.loads(os.environ["ITEM_JSON"])
print(f"- {item['status']}: `{item['name']}` — `{item['command']}` ({item['log']})")
PY
        done
    } > "$SUMMARY_MD"

    {
        local total_count
        total_count="${#RESULT_NAMES[@]}"
        printf '<testsuites>\n'
        printf '  <testsuite name="%s" tests="%s" failures="%s" skipped="%s" timestamp="%s">\n' \
            "$(xml_escape "clankers test-harness $MODE")" \
            "$total_count" \
            "$FAIL_COUNT" \
            "$SKIP_COUNT" \
            "$(xml_escape "$STARTED_AT")"
        local index
        for index in "${!RESULT_NAMES[@]}"; do
            local name status exit_code log_file command_text
            name="${RESULT_NAMES[$index]}"
            status="${RESULT_STATUSES[$index]}"
            exit_code="${RESULT_EXIT_CODES[$index]}"
            log_file="${RESULT_LOGS[$index]}"
            command_text="${RESULT_COMMANDS[$index]}"
            printf '    <testcase classname="%s" name="%s">\n' \
                "$(xml_escape "test-harness.$MODE")" \
                "$(xml_escape "$name")"
            if [[ "$status" == "failed" ]]; then
                printf '      <failure message="%s">command: %s&#10;log: %s&#10;exit_code: %s</failure>\n' \
                    "$(xml_escape "exit code $exit_code")" \
                    "$(xml_escape "$command_text")" \
                    "$(xml_escape "$log_file")" \
                    "$(xml_escape "$exit_code")"
            elif [[ "$status" == "skipped" ]]; then
                printf '      <skipped message="%s"/>\n' \
                    "$(xml_escape "dry run")"
            fi
            printf '    </testcase>\n'
        done
        printf '  </testsuite>\n'
        printf '</testsuites>\n'
    } > "$JUNIT_XML"

    echo "summary: $SUMMARY_MD"
    echo "json:    $RESULTS_JSON"
    echo "junit:   $JUNIT_XML"
}

main() {
    case "$MODE" in
        quick)
            run_step "cargo check tests" cargo check --tests
            run_step "cargo nextest workspace" cargo nextest run --workspace --no-fail-fast
            ;;
        package)
            local package="${1:-}"
            if [[ -z "$package" ]]; then
                echo "error: package mode requires a crate/package name" >&2
                usage >&2
                return 2
            fi
            shift || true
            run_step "cargo check $package" cargo check --tests -p "$package"
            if [[ $# -gt 0 ]]; then
                run_step "cargo nextest $package filtered" cargo nextest run -p "$package" --no-fail-fast "$@"
            else
                run_step "cargo nextest $package" cargo nextest run -p "$package" --no-fail-fast
            fi
            ;;
        full)
            run_step "cargo fmt check" cargo fmt --check
            run_step "cargo check tests" cargo check --tests
            run_step "cargo nextest workspace" cargo nextest run --workspace --no-fail-fast
            run_step "cargo clippy" cargo clippy --workspace --all-targets -- -D warnings
            run_step "repo verify" ./scripts/verify.sh
            run_step "tigerstyle" ./xtask/tigerstyle.sh
            ;;
        e2e)
            local selector="${1:-all}"
            run_step "e2e $selector" ./tests/e2e/run-tests.sh "$selector"
            ;;
        vm)
            run_vm_selector "${1:-all}"
            ;;
        ci)
            run_step "nix flake check" nix flake check "$@"
            ;;
        help|-h|--help)
            usage
            ;;
        *)
            echo "error: unknown mode: $MODE" >&2
            usage >&2
            return 2
            ;;
    esac
}

trap write_reports EXIT
main "$@"
