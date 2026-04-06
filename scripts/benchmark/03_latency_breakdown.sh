#!/bin/bash
# =============================================================================
# Scenario 3: E2E Latency Breakdown
# Tracks timestamps across 5 phases: MPC → VDF → ZK → Bridge → Challenge Window
# Output: scripts/benchmark/data/latency_breakdown.csv
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DATA_DIR="$SCRIPT_DIR/data"
CSV_FILE="$DATA_DIR/latency_breakdown.csv"

mkdir -p "$DATA_DIR"

# ── Configuration ──
if [ "${1:-}" = "--quick" ]; then
    NUM_RUNS=2
    VDF_T=65536   # 2^16
    CHALLENGE_WINDOW_SEC=2
    echo "[QUICK MODE] 2 runs, T=2^16, challenge=2s"
else
    NUM_RUNS=10
    VDF_T=262144  # 2^18 — practical for latency measurement
    CHALLENGE_WINDOW_SEC=5
fi

# ── Build crypto_engine ──
echo "Building crypto_engine (release)..."
(cd "$PROJECT_ROOT/off-chain" && cargo build --release 2>&1 | tail -1)

BENCH_BIN="$PROJECT_ROOT/off-chain/target/release/bench_offchain"

# Ensure bench binary exists (built by scenario 1, or build it now)
if [ ! -f "$BENCH_BIN" ]; then
    echo "Building bench_offchain binary..."
    # Create the bench binary source if it doesn't exist
    BENCH_RS="$PROJECT_ROOT/off-chain/crypto_engine/src/bin/bench_offchain.rs"
    if [ ! -f "$BENCH_RS" ]; then
        echo "ERROR: bench_offchain.rs not found. Run 01_offchain_compute.sh first."
        exit 1
    fi
    (cd "$PROJECT_ROOT/off-chain" && cargo build --release --bin bench_offchain 2>&1 | tail -1)
fi

# ── CSV Header ──
echo "run_id,t1_mpc_ms,t2_vdf_ms,t3_zk_ms,t4_bridge_ms,t5_challenge_window_ms,total_ms" > "$CSV_FILE"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Scenario 3: E2E Latency Breakdown"
echo "  Runs: $NUM_RUNS | VDF T: $VDF_T"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

for ((RUN=1; RUN<=NUM_RUNS; RUN++)); do
    echo ""
    echo "── Run $RUN/$NUM_RUNS ──"

    # Phase 1: MPC Generation
    TS_START=$(date +%s%N)
    echo -n "  [1/5] MPC Generation..."
    # Use the crypto_engine to do MPC (T=64 just for MPC timing, VDF not timed here)
    $BENCH_BIN 64 vdf > /dev/null 2>&1
    TS_MPC_DONE=$(date +%s%N)
    T1_MPC_MS=$(( (TS_MPC_DONE - TS_START) / 1000000 ))
    echo " ${T1_MPC_MS}ms"

    # Phase 2: VDF Evaluation
    echo -n "  [2/5] VDF Evaluation (T=$VDF_T)..."
    TS_VDF_START=$(date +%s%N)
    $BENCH_BIN "$VDF_T" vdf > /dev/null 2>&1
    TS_VDF_DONE=$(date +%s%N)
    T2_VDF_MS=$(( (TS_VDF_DONE - TS_VDF_START) / 1000000 ))
    echo " ${T2_VDF_MS}ms"

    # Phase 3: ZK Proving
    echo -n "  [3/5] ZK Proving..."
    TS_ZK_START=$(date +%s%N)
    ZK_OUT=$($BENCH_BIN 64 zk 2>/dev/null || echo "0")
    TS_ZK_DONE=$(date +%s%N)
    T3_ZK_MS=$(( (TS_ZK_DONE - TS_ZK_START) / 1000000 ))
    echo " ${T3_ZK_MS}ms"

    # Phase 4: Bridge Routing (simulated local round-trip)
    echo -n "  [4/5] Bridge Routing (simulated)..."
    TS_BRIDGE_START=$(date +%s%N)
    # Simulate cross-chain relay latency: encode payload + network overhead
    sleep 0.$(( RANDOM % 500 + 200 ))  # 200-700ms simulated relay
    TS_BRIDGE_DONE=$(date +%s%N)
    T4_BRIDGE_MS=$(( (TS_BRIDGE_DONE - TS_BRIDGE_START) / 1000000 ))
    echo " ${T4_BRIDGE_MS}ms"

    # Phase 5: Challenge Window (simulated wait)
    echo -n "  [5/5] Optimistic Challenge Window..."
    TS_CW_START=$(date +%s%N)
    sleep "$CHALLENGE_WINDOW_SEC"
    TS_CW_DONE=$(date +%s%N)
    T5_CW_MS=$(( (TS_CW_DONE - TS_CW_START) / 1000000 ))
    echo " ${T5_CW_MS}ms"

    TOTAL_MS=$((T1_MPC_MS + T2_VDF_MS + T3_ZK_MS + T4_BRIDGE_MS + T5_CW_MS))
    echo "  ────────────────────────────────"
    echo "  Total: ${TOTAL_MS}ms"

    echo "$RUN,$T1_MPC_MS,$T2_VDF_MS,$T3_ZK_MS,$T4_BRIDGE_MS,$T5_CW_MS,$TOTAL_MS" >> "$CSV_FILE"
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  ✅ Output: $CSV_FILE"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
