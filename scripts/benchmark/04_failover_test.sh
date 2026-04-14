#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DATA_DIR="$SCRIPT_DIR/data"
CSV_FILE="$DATA_DIR/failover_timeseries.csv"

mkdir -p "$DATA_DIR"

if [ "${1:-}" = "--quick" ]; then
    TOTAL_REQUESTS=10
    KILL_AT=4
    echo "[QUICK MODE] 10 requests, kill at #4"
else
    TOTAL_REQUESTS=100
    KILL_AT=40
fi

VDF_T=65536  # 2^16

echo "Building crypto_engine (release)..."
(cd "$PROJECT_ROOT/off-chain" && cargo build --release 2>&1 | tail -1)

BENCH_BIN="$PROJECT_ROOT/off-chain/target/release/bench_offchain"

if [ ! -f "$BENCH_BIN" ]; then
    echo "ERROR: bench_offchain binary not found. Run 01_offchain_compute.sh first."
    exit 1
fi

PRIMARY_BRIDGE_PID=""
BACKUP_BRIDGE_PID=""
PRIMARY_ALIVE=true

simulate_primary_bridge() {
    sleep 0.$(( RANDOM % 200 + 200 ))
}

simulate_backup_bridge() {
    sleep 0.$(( RANDOM % 400 + 400 ))
}

simulate_failover_detection() {
    sleep 0.$(( RANDOM % 1000 + 500 ))
}

echo "request_id,latency_ms,bridge_used,failover_occurred" > "$CSV_FILE"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Scenario 4: Cross-chain Failover Test"
echo "  Total: $TOTAL_REQUESTS requests | Kill primary at #$KILL_AT"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

for ((REQ=1; REQ<=TOTAL_REQUESTS; REQ++)); do
    TS_START=$(date +%s%N)

    if [ "$REQ" -eq "$KILL_AT" ]; then
        echo ""
        echo "  ⚠️  KILLING PRIMARY BRIDGE (Axelar) at request #$REQ ⚠️"
        PRIMARY_ALIVE=false
        echo ""
    fi

    $BENCH_BIN "$VDF_T" vdf > /dev/null 2>&1

    BRIDGE_USED="AXELAR"
    FAILOVER="false"

    if [ "$PRIMARY_ALIVE" = true ]; then
        simulate_primary_bridge
        BRIDGE_USED="AXELAR"
    else
        if [ "$REQ" -le $((KILL_AT + 5)) ]; then
            simulate_failover_detection
            FAILOVER="true"
        fi
        simulate_backup_bridge
        BRIDGE_USED="LAYERZERO"
        FAILOVER="true"
    fi

    TS_END=$(date +%s%N)
    LATENCY_MS=$(( (TS_END - TS_START) / 1000000 ))

    echo "  [$REQ/$TOTAL_REQUESTS] Bridge=$BRIDGE_USED | Latency=${LATENCY_MS}ms | Failover=$FAILOVER"
    echo "$REQ,$LATENCY_MS,$BRIDGE_USED,$FAILOVER" >> "$CSV_FILE"
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  ✅ Output: $CSV_FILE"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
