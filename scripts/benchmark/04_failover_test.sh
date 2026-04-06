#!/bin/bash
# =============================================================================
# Scenario 4: Cross-chain Failover Test
# Sends 100 simulated requests. At request #40, kills primary bridge.
# Measures per-request latency to show failover spike + recovery.
# Output: scripts/benchmark/data/failover_timeseries.csv
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DATA_DIR="$SCRIPT_DIR/data"
CSV_FILE="$DATA_DIR/failover_timeseries.csv"

mkdir -p "$DATA_DIR"

# ── Configuration ──
if [ "${1:-}" = "--quick" ]; then
    TOTAL_REQUESTS=10
    KILL_AT=4
    echo "[QUICK MODE] 10 requests, kill at #4"
else
    TOTAL_REQUESTS=100
    KILL_AT=40
fi

# VDF params for each request (small T for speed)
VDF_T=65536  # 2^16

# ── Build crypto_engine ──
echo "Building crypto_engine (release)..."
(cd "$PROJECT_ROOT/off-chain" && cargo build --release 2>&1 | tail -1)

BENCH_BIN="$PROJECT_ROOT/off-chain/target/release/bench_offchain"

if [ ! -f "$BENCH_BIN" ]; then
    echo "ERROR: bench_offchain binary not found. Run 01_offchain_compute.sh first."
    exit 1
fi

# ── Bridge Simulation ──
# We simulate primary (Axelar) and backup (LayerZero) bridges as processes
# Primary bridge: fast relay (~200-400ms)
# Backup bridge: slightly slower (~400-800ms), plus failover detection overhead

PRIMARY_BRIDGE_PID=""
BACKUP_BRIDGE_PID=""
PRIMARY_ALIVE=true

simulate_primary_bridge() {
    # Simulated Axelar relay: 200-400ms
    sleep 0.$(( RANDOM % 200 + 200 ))
}

simulate_backup_bridge() {
    # Simulated LayerZero relay: 400-800ms (slower fallback)
    sleep 0.$(( RANDOM % 400 + 400 ))
}

simulate_failover_detection() {
    # Time to detect primary failure and switch: 500-1500ms
    sleep 0.$(( RANDOM % 1000 + 500 ))
}

# ── CSV Header ──
echo "request_id,latency_ms,bridge_used,failover_occurred" > "$CSV_FILE"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Scenario 4: Cross-chain Failover Test"
echo "  Total: $TOTAL_REQUESTS requests | Kill primary at #$KILL_AT"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

for ((REQ=1; REQ<=TOTAL_REQUESTS; REQ++)); do
    TS_START=$(date +%s%N)

    # At KILL_AT, simulate primary bridge death
    if [ "$REQ" -eq "$KILL_AT" ]; then
        echo ""
        echo "  ⚠️  KILLING PRIMARY BRIDGE (Axelar) at request #$REQ ⚠️"
        PRIMARY_ALIVE=false
        echo ""
    fi

    # Run VDF computation (constant baseline per request)
    $BENCH_BIN "$VDF_T" vdf > /dev/null 2>&1

    BRIDGE_USED="AXELAR"
    FAILOVER="false"

    if [ "$PRIMARY_ALIVE" = true ]; then
        # Normal path: use primary bridge
        simulate_primary_bridge
        BRIDGE_USED="AXELAR"
    else
        # Primary is dead: attempt primary, fail, detect, fallback
        if [ "$REQ" -le $((KILL_AT + 5)) ]; then
            # Transition period: failover detection adds overhead
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
