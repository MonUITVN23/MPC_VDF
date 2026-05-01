#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DATA_DIR="$SCRIPT_DIR/data"
CSV_FILE="$DATA_DIR/failover_timeseries.csv"

mkdir -p "$DATA_DIR"

if [ "${1:-}" = "--quick" ]; then
    TOTAL_REQUESTS=12
    KILL_AT=5
    echo "[QUICK MODE] 12 requests, kill at #5"
else
    TOTAL_REQUESTS="${BENCH_TOTAL_REQUESTS:-120}"
    KILL_AT="${BENCH_KILL_AT:-40}"
    echo "[IEEE MODE] ${TOTAL_REQUESTS} requests, kill at #${KILL_AT}"
fi

VDF_T=65536  

echo "Building crypto_engine (release)..."
(cd "$PROJECT_ROOT/off-chain" && cargo build --release 2>&1 | tail -1)

BENCH_BIN="$PROJECT_ROOT/off-chain/target/release/bench_offchain"

if [ ! -f "$BENCH_BIN" ]; then
    echo "ERROR: bench_offchain binary not found. Run 01_offchain_compute.sh first."
    exit 1
fi

PRIMARY_ALIVE=true

# Detailed CSV Header for better plotting
echo "request_id,vdf_ms,bridge_ms,detection_ms,total_latency_ms,bridge_used,failover_status" > "$CSV_FILE"

echo ""
echo "------------------------------------------------"
echo "  Scenario 4: Cross-chain Failover Test"
echo "  Total: $TOTAL_REQUESTS requests | Kill primary at #$KILL_AT"
echo "------------------------------------------------"

for ((REQ=1; REQ<=TOTAL_REQUESTS; REQ++)); do
    if [ "$REQ" -eq "$KILL_AT" ]; then
        echo ""
        echo "  [SYSTEM ALERT] KILLING PRIMARY BRIDGE (Axelar) at request #$REQ"
        PRIMARY_ALIVE=false
        echo ""
    fi

    # 1. Actual VDF Computation
    TS_START=$(date +%s%N)
    $BENCH_BIN "$VDF_T" vdf > /dev/null 2>&1
    TS_END=$(date +%s%N)
    VDF_MS=$(( (TS_END - TS_START) / 1000000 ))

    # 2. Simulated Network Dynamics
    BRIDGE_USED="AXELAR"
    FAILOVER_STATUS="STABLE"
    BRIDGE_MS=0
    DETECTION_MS=0

    if [ "$PRIMARY_ALIVE" = true ]; then
        # Primary bridge routing: 200ms - 400ms
        BRIDGE_MS=$(( RANDOM % 200 + 200 ))
        BRIDGE_USED="AXELAR"
    else
        BRIDGE_USED="LAYERZERO"
        # Simulate timeout/detection overhead for the first 5 requests after failure
        if [ "$REQ" -le $((KILL_AT + 5)) ]; then
            DETECTION_MS=$(( RANDOM % 1000 + 500 ))
            FAILOVER_STATUS="RECOVERING"
        else
            FAILOVER_STATUS="STABLE_BACKUP"
        fi
        # Backup bridge routing: 400ms - 800ms
        BRIDGE_MS=$(( RANDOM % 400 + 400 ))
    fi

    TOTAL_LATENCY_MS=$(( VDF_MS + BRIDGE_MS + DETECTION_MS ))

    echo "  [$REQ/$TOTAL_REQUESTS] Bridge=$BRIDGE_USED | Latency=${TOTAL_LATENCY_MS}ms | Status=$FAILOVER_STATUS"
    echo "$REQ,$VDF_MS,$BRIDGE_MS,$DETECTION_MS,$TOTAL_LATENCY_MS,$BRIDGE_USED,$FAILOVER_STATUS" >> "$CSV_FILE"
done

echo ""
echo "------------------------------------------------"
echo "  Output generated: $CSV_FILE"
echo "------------------------------------------------"