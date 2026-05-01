#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DATA_DIR="$SCRIPT_DIR/data"
CSV_FILE="$DATA_DIR/latency_breakdown.csv"

mkdir -p "$DATA_DIR"

if [ "${1:-}" = "--quick" ]; then
    NUM_RUNS=3
    VDF_T=65536   
    CHALLENGE_WINDOW_SEC=2
    echo "[QUICK MODE] 3 runs, T=2^16, challenge=2s"
else
    NUM_RUNS="${BENCH_NUM_RUNS:-30}"
    VDF_T=262144  
    CHALLENGE_WINDOW_SEC=5
    echo "[IEEE MODE] ${NUM_RUNS} runs, T=2^18, challenge=5s"
fi

echo "Building crypto_engine (release)..."
(cd "$PROJECT_ROOT/off-chain" && cargo build --release 2>&1 | tail -1)

BENCH_BIN="$PROJECT_ROOT/off-chain/target/release/bench_offchain"

if [ ! -f "$BENCH_BIN" ]; then
    echo "ERROR: bench_offchain binary not found. Run 01_offchain_compute.sh first."
    exit 1
fi

# Removed ZK from CSV Header
echo "run_id,t1_mpc_ms,t2_vdf_ms,t3_bridge_ms,t4_challenge_window_ms,total_critical_ms" > "$CSV_FILE"

echo ""
echo "------------------------------------------------"
echo "  Scenario 3: E2E Latency Breakdown"
echo "  Runs: $NUM_RUNS | VDF T: $VDF_T"
echo "------------------------------------------------"

for ((RUN=1; RUN<=NUM_RUNS; RUN++)); do
    echo ""
    echo "-- Run $RUN/$NUM_RUNS --"

    # Step 1: Simulated MPC network latency
    echo -n "  [1/4] MPC Generation (simulated network)..."
    T1_MPC_MS=$(( RANDOM % 60 + 120 ))
    echo " ${T1_MPC_MS}ms"

    # Step 2: Actual VDF Evaluation on local CPU
    echo -n "  [2/4] VDF Evaluation (T=$VDF_T)..."
    TS_VDF_START=$(date +%s%N)
    $BENCH_BIN "$VDF_T" vdf > /dev/null 2>&1
    TS_VDF_DONE=$(date +%s%N)
    T2_VDF_MS=$(( (TS_VDF_DONE - TS_VDF_START) / 1000000 ))
    echo " ${T2_VDF_MS}ms"

    # Step 3: Simulated Bridge Routing latency
    echo -n "  [3/4] Bridge Routing (simulated)..."
    T3_BRIDGE_MS=$(( RANDOM % 300 + 400 ))
    echo " ${T3_BRIDGE_MS}ms"

    # Step 4: Fast-forward Challenge Window
    echo -n "  [4/4] Optimistic Challenge Window..."
    T4_CW_MS=$(( CHALLENGE_WINDOW_SEC * 1000 ))
    echo " ${T4_CW_MS}ms"

    # Total E2E Latency (Optimistic Happy Path)
    CRITICAL_PATH_MS=$((T1_MPC_MS + T2_VDF_MS + T3_BRIDGE_MS + T4_CW_MS))
    
    echo "  --------------------------------"
    echo "  Total E2E Latency (Happy Path) : ${CRITICAL_PATH_MS}ms"

    echo "$RUN,$T1_MPC_MS,$T2_VDF_MS,$T3_BRIDGE_MS,$T4_CW_MS,$CRITICAL_PATH_MS" >> "$CSV_FILE"
done

echo ""
echo "------------------------------------------------"
echo "  Output: $CSV_FILE"
echo "------------------------------------------------"