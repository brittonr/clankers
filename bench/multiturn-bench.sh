#!/usr/bin/env bash
# multiturn-bench.sh — Multi-turn context growth benchmark
#
# Sends a single prompt that forces the agent to read many files sequentially,
# creating many tool_result messages in context. Measures how input tokens grow
# across turns within that session.
#
# With compaction: old tool results get replaced with summaries → sub-linear growth.
# Without compaction: full tool results stay in context → linear growth.
#
# Usage:
#   ./bench/multiturn-bench.sh                  # full run (clankers + pi)
#   ./bench/multiturn-bench.sh --clankers-only  # clankers only
#   ./bench/multiturn-bench.sh --pi-only        # pi only

set -euo pipefail

SCRIPT_DIR="$(unset CDPATH; cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"
CLANKERS_BIN="${CLANKERS_BIN:-${CARGO_TARGET_DIR:-$REPO_DIR/target}/debug/clankers}"
PI_BIN="${PI_BIN:-pi}"
MODEL="${BENCH_MODEL:-claude-sonnet-4-20250514}"
WORKDIR="${BENCH_WORKDIR:-$REPO_DIR}"
TIMEOUT=300
PI_ONLY=false
CLANKERS_ONLY=false

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

while [[ $# -gt 0 ]]; do
    case $1 in
        --model)         MODEL="$2"; shift 2 ;;
        --pi-only)       PI_ONLY=true; shift ;;
        --clankers-only) CLANKERS_ONLY=true; shift ;;
        --workdir)       WORKDIR="$2"; shift 2 ;;
        --timeout)       TIMEOUT="$2"; shift 2 ;;
        --help|-h)
            echo "Usage: $0 [--clankers-only|--pi-only] [--model MODEL]"
            exit 0 ;;
        *) echo "Unknown: $1"; exit 1 ;;
    esac
done

TIMESTAMP=$(date +%Y%m%d-%H%M%S)
RESULTS_DIR="/tmp/multiturn-bench/$TIMESTAMP"
mkdir -p "$RESULTS_DIR"

# ── The prompt ────────────────────────────────────────────────────────
# Forces many sequential file reads to build up tool results in context.
# We ask for specific data from each file so the agent can't skip reads.
PROMPT='Read each of these files ONE AT A TIME (do not batch them) and for each file report: (1) the filename, (2) its line count, (3) the first function/struct name you see. Read them in this exact order:

1. Cargo.toml
2. src/main.rs
3. crates/clankers-agent/src/lib.rs
4. crates/clankers-agent/src/context.rs
5. crates/clankers-agent/src/system_prompt.rs
6. crates/clankers-agent/src/tool.rs
7. crates/clankers-loop/src/lib.rs
8. crates/clankers-loop/src/truncation.rs
9. crates/clankers-db/src/lib.rs
10. crates/clankers-db/src/tool_results.rs
11. crates/clankers-session/src/entry.rs
12. crates/clankers-router/src/lib.rs

After reading all 12, produce a summary table. Important: read each file individually with the read tool, do NOT use bash/grep/find shortcuts.'

echo -e "${BOLD}Multi-Turn Context Growth Benchmark${RESET}"
echo -e "${DIM}────────────────────────────────────────────${RESET}"
echo -e "  Model:     ${CYAN}$MODEL${RESET}"
echo -e "  Strategy:  ${CYAN}Single prompt forcing 12+ sequential file reads${RESET}"
echo -e "  Workdir:   ${CYAN}$WORKDIR${RESET}"
echo -e "  Results:   ${CYAN}$RESULTS_DIR${RESET}"

if ! $CLANKERS_ONLY; then
    if ! command -v "$PI_BIN" &>/dev/null; then
        echo -e "${RED}pi not found${RESET}"; exit 1
    fi
    echo -e "  pi:        ${GREEN}$(command -v "$PI_BIN")${RESET}"
fi
if ! $PI_ONLY; then
    if [[ ! -x "$CLANKERS_BIN" ]]; then
        echo -e "${YELLOW}Building clankers...${RESET}"
        (cd "$REPO_DIR" && cargo build 2>/dev/null)
    fi
    echo -e "  clankers:  ${GREEN}$CLANKERS_BIN${RESET}"
fi
echo -e "${DIM}────────────────────────────────────────────${RESET}"
echo ""

# ── Run clankers ──────────────────────────────────────────────────────
run_agent() {
    local agent="$1"
    local outfile="$2"

    echo -ne "  Running ${BLUE}${agent}${RESET}... "
    local start_time=$(date +%s%N)

    if [[ "$agent" == "clankers" ]]; then
        (cd "$WORKDIR" && timeout "$TIMEOUT" "$CLANKERS_BIN" \
            -p "$PROMPT" \
            --mode json \
            --model "$MODEL" \
            --auto-approve \
            --max-iterations 30 \
            --no-session \
            2>/dev/null > "$outfile") || true
    else
        (cd "$WORKDIR" && timeout "$TIMEOUT" "$PI_BIN" \
            --mode json \
            -p "$PROMPT" \
            --model "$MODEL" \
            --no-skills --no-extensions --no-prompt-templates \
            --no-session \
            2>/dev/null > "$outfile") || true
    fi

    local end_time=$(date +%s%N)
    local elapsed_s=$(( (end_time - start_time) / 1000000000 ))
    echo -e "${GREEN}done${RESET} (${elapsed_s}s)"
}

# ── Extract per-turn usage ────────────────────────────────────────────
analyze_turns() {
    local jsonl="$1"
    local agent="$2"
    local out_json="$3"

    python3 - "$jsonl" "$agent" "$out_json" <<'PYEOF'
import json, sys

jsonl_path = sys.argv[1]
agent = sys.argv[2]
out_path = sys.argv[3]

turns = []

with open(jsonl_path) as f:
    for line in f:
        try:
            evt = json.loads(line.strip())

            if agent == "clankers" and evt.get("type") == "usage":
                turn = evt.get("turn", {})
                cum = evt.get("cumulative", {})
                turns.append({
                    "turn": len(turns) + 1,
                    "input": turn.get("input_tokens", 0),
                    "output": turn.get("output_tokens", 0),
                    "cache_read": turn.get("cache_read_tokens", 0),
                    "cache_create": turn.get("cache_create_tokens", 0),
                    "cum_input": cum.get("input_tokens", 0),
                    "cum_output": cum.get("output_tokens", 0),
                })

            elif agent == "pi" and evt.get("type") == "message_end":
                msg = evt.get("message", {})
                if msg.get("role") == "assistant":
                    u = msg.get("usage", {})
                    if u:
                        turns.append({
                            "turn": len(turns) + 1,
                            "input": u.get("input", u.get("input_tokens", 0)),
                            "output": u.get("output", u.get("output_tokens", 0)),
                            "cache_read": u.get("cacheRead", u.get("cache_read", 0)),
                            "cache_create": u.get("cacheWrite", u.get("cache_write", 0)),
                            "cum_input": u.get("input", u.get("input_tokens", 0)),
                            "cum_output": u.get("output", u.get("output_tokens", 0)),
                        })

            elif agent == "pi" and evt.get("type") == "result":
                u = evt.get("usage", {})
                if u:
                    turns.append({
                        "turn": len(turns) + 1,
                        "input": u.get("input", u.get("input_tokens", 0)),
                        "output": u.get("output", u.get("output_tokens", 0)),
                        "cache_read": u.get("cacheRead", u.get("cache_read", 0)),
                        "cache_create": u.get("cacheWrite", u.get("cache_write", 0)),
                        "cum_input": u.get("input", u.get("input_tokens", 0)),
                        "cum_output": u.get("output", u.get("output_tokens", 0)),
                    })
        except:
            pass

with open(out_path, "w") as f:
    json.dump(turns, f, indent=2)
PYEOF
}

# ── Print analysis ────────────────────────────────────────────────────
print_analysis() {
    local ck_json="$1"
    local pi_json="$2"

    python3 - "$ck_json" "$pi_json" <<'PYEOF'
import json, sys

def load(path):
    try:
        with open(path) as f:
            return json.load(f)
    except:
        return []

ck = load(sys.argv[1])
pi = load(sys.argv[2])

# Anthropic pricing (per MTok)
PRICE_INPUT = 3.00
PRICE_CACHE_WRITE = 3.75
PRICE_CACHE_READ = 0.30
PRICE_OUTPUT = 15.00

def compute_cost(t):
    """Compute cost for a single turn using Anthropic pricing."""
    inp = t.get("input", 0)
    out = t.get("output", 0)
    cr = t.get("cache_read", 0)
    cw = t.get("cache_create", 0)
    # input_tokens from API = non-cached input. Total context = input + cache_read + cache_create
    cost = (inp * PRICE_INPUT + cw * PRICE_CACHE_WRITE + cr * PRICE_CACHE_READ + out * PRICE_OUTPUT) / 1_000_000
    return cost

def total_context(t):
    """Total context size = input + cache_read + cache_create."""
    return t.get("input", 0) + t.get("cache_read", 0) + t.get("cache_create", 0)

def print_growth(turns, label):
    if not turns:
        print(f"  {label}: no data")
        return []

    print(f"\n  \033[1m{label}\033[0m ({len(turns)} turns)")
    print(f"  {'Turn':>5} {'Context':>9} {'Input':>8} {'CacheRd':>9} {'CacheWr':>9} {'Output':>7} {'Δ Ctx':>9} {'Cost':>8}")
    print(f"  {'─'*5} {'─'*9} {'─'*8} {'─'*9} {'─'*9} {'─'*7} {'─'*9} {'─'*8}")

    prev_ctx = 0
    contexts = []
    total_cost = 0.0
    for t in turns:
        ctx = total_context(t)
        inp = t["input"]
        out = t["output"]
        cr = t.get("cache_read", 0)
        cw = t.get("cache_create", 0)
        cost = compute_cost(t)
        total_cost += cost
        delta = ctx - prev_ctx if prev_ctx > 0 else 0

        # Color growth rate
        color = ""
        if prev_ctx > 0 and len(contexts) >= 2:
            prev_delta = contexts[-1] - (contexts[-2] if len(contexts) >= 2 else 0)
            if prev_delta > 0 and delta < prev_delta * 0.7:
                color = "\033[32m"  # decelerating
            elif prev_delta > 0 and delta > prev_delta * 1.3:
                color = "\033[31m"  # accelerating

        print(f"  {t['turn']:>5} {ctx:>9,} {inp:>8,} {cr:>9,} {cw:>9,} {out:>7,} {color}{delta:>+9,}\033[0m ${cost:>7.4f}")
        prev_ctx = ctx
        contexts.append(ctx)

    # Summary
    if len(contexts) >= 2:
        ratio = contexts[-1] / contexts[0] if contexts[0] > 0 else 0
        print(f"\n  Context: {contexts[0]:,} → {contexts[-1]:,} (×{ratio:.1f} over {len(contexts)} turns)")
        print(f"  Total cost: ${total_cost:.4f}")

        # Growth deceleration check
        if len(contexts) >= 6:
            mid = len(contexts) // 2
            early_deltas = [contexts[i] - contexts[i-1] for i in range(1, mid+1) if contexts[i] > contexts[i-1]]
            late_deltas = [contexts[i] - contexts[i-1] for i in range(mid+1, len(contexts)) if contexts[i] > contexts[i-1]]
            if early_deltas and late_deltas:
                early_avg = sum(early_deltas) / len(early_deltas)
                late_avg = sum(late_deltas) / len(late_deltas)
                if early_avg > 0:
                    decel = late_avg / early_avg
                    if decel < 0.8:
                        print(f"  \033[32mGrowth rate: early +{early_avg:,.0f}/turn → late +{late_avg:,.0f}/turn (×{decel:.2f})\033[0m")
                        print(f"  → Compaction saved ~{(1-decel)*100:.0f}% growth per turn in second half")
                        # Estimate tokens saved vs linear
                        linear_final = contexts[0] + early_avg * (len(contexts) - 1)
                        saved = linear_final - contexts[-1]
                        if saved > 0:
                            print(f"  → Estimated {saved:,.0f} tokens saved vs linear growth ({saved/linear_final*100:.0f}%)")
                    elif decel > 1.2:
                        print(f"  \033[31mGrowth rate: early +{early_avg:,.0f}/turn → late +{late_avg:,.0f}/turn (×{decel:.2f})\033[0m")
                    else:
                        print(f"  Growth rate: early +{early_avg:,.0f}/turn → late +{late_avg:,.0f}/turn (×{decel:.2f})")

    return contexts, total_cost

print(f"\n\033[1m{'═' * 82}\033[0m")
print(f"\033[1m  MULTI-TURN CONTEXT GROWTH ANALYSIS\033[0m")
print(f"\033[1m{'═' * 82}\033[0m")
print(f"  'Context' = input + cache_read + cache_create (total tokens sent to model).")
print(f"  Cost uses Anthropic Sonnet pricing: $3/MTok input, $0.30 cache read, $15 output.")
print(f"  With compaction, context growth should decelerate as old tool results shrink.")

ck_result = print_growth(ck, "clankers")
pi_result = print_growth(pi, "pi")

ck_contexts = ck_result[0] if ck_result else []
ck_cost = ck_result[1] if ck_result else 0
pi_contexts = pi_result[0] if pi_result else []
pi_cost = pi_result[1] if pi_result else 0

# Side-by-side
if ck and pi:
    max_turns = max(len(ck), len(pi))
    print(f"\n  \033[1mSide-by-side (total context per turn)\033[0m")
    print(f"  {'Turn':>5} {'clankers':>12} {'pi':>12} {'Δ':>10}")
    print(f"  {'─'*5} {'─'*12} {'─'*12} {'─'*10}")
    for i in range(max_turns):
        ck_ctx = total_context(ck[i]) if i < len(ck) else None
        pi_ctx = total_context(pi[i]) if i < len(pi) else None
        ck_str = f"{ck_ctx:,}" if ck_ctx is not None else "—"
        pi_str = f"{pi_ctx:,}" if pi_ctx is not None else "—"
        if ck_ctx and pi_ctx and pi_ctx > 0:
            diff = ((ck_ctx - pi_ctx) / pi_ctx) * 100
            color = "\033[32m" if diff < 0 else ("\033[31m" if diff > 0 else "")
            diff_str = f"{color}{diff:+.0f}%\033[0m"
        else:
            diff_str = "—"
        print(f"  {i+1:>5} {ck_str:>12} {pi_str:>12} {diff_str:>10}")

    print(f"\n  \033[1mCost comparison:\033[0m")
    print(f"    clankers: ${ck_cost:.4f}")
    print(f"    pi:       ${pi_cost:.4f}")
    if pi_cost > 0:
        savings = ((pi_cost - ck_cost) / pi_cost) * 100
        if savings > 0:
            print(f"    \033[32m→ clankers {savings:.0f}% cheaper\033[0m")
        else:
            print(f"    \033[31m→ clankers {-savings:.0f}% more expensive\033[0m")

# Save summary
summary = {"clankers": ck, "pi": pi}
out_path = sys.argv[1].replace("ck_turns", "summary").replace("pi_turns", "summary")
with open(out_path, "w") as f:
    json.dump(summary, f, indent=2)
print(f"\n  Summary: {out_path}")
print()
PYEOF
}

# ── Main ──────────────────────────────────────────────────────────────
if ! $PI_ONLY; then
    run_agent "clankers" "$RESULTS_DIR/ck_raw.jsonl"
    analyze_turns "$RESULTS_DIR/ck_raw.jsonl" "clankers" "$RESULTS_DIR/ck_turns.json"
fi

if ! $CLANKERS_ONLY; then
    run_agent "pi" "$RESULTS_DIR/pi_raw.jsonl"
    analyze_turns "$RESULTS_DIR/pi_raw.jsonl" "pi" "$RESULTS_DIR/pi_turns.json"
fi

print_analysis "$RESULTS_DIR/ck_turns.json" "$RESULTS_DIR/pi_turns.json"

echo -e "${DIM}Raw data: $RESULTS_DIR${RESET}"
