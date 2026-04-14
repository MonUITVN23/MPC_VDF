#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DATA_DIR="$SCRIPT_DIR/data"
CSV_FILE="$DATA_DIR/offchain_compute.csv"

mkdir -p "$DATA_DIR"

if [ "${1:-}" = "--quick" ]; then
    T_EXPONENTS=(16 17)
    echo "[QUICK MODE] Only 2 data points for smoke test."
else
    T_EXPONENTS=(16 17 18 19 20 21)
fi

GNU_TIME=""
if command -v /usr/bin/time &>/dev/null; then
    GNU_TIME="/usr/bin/time"
elif command -v gtime &>/dev/null; then
    GNU_TIME="gtime"
else
    echo "WARNING: GNU time not found. RAM stats will be 0."
fi

echo "Building crypto_engine (release)..."
(cd "$PROJECT_ROOT/off-chain" && cargo build --release 2>&1 | tail -1)

echo "T,T_exp,vdf_ms,zk_prove_ms,peak_rss_kb,cpu_percent" > "$CSV_FILE"

BENCH_RS="$PROJECT_ROOT/off-chain/crypto_engine/src/bin/bench_offchain.rs"
mkdir -p "$(dirname "$BENCH_RS")"

cat > "$BENCH_RS" << 'RUSTEOF'
use std::env;
use std::time::Instant;
use crypto_engine::{mpc, vdf};

fn main() {
    let args: Vec<String> = env::args().collect();
    let t: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(65536);
    let mode = args.get(2).map(|s| s.as_str()).unwrap_or("vdf");

    // Generate MPC seed
    let collective = mpc::init_collective_seed_default().expect("MPC failed");
    let seed: [u8; 32] = {
        use sha2::{Sha256, Digest};
        let mut h = Sha256::new();
        h.update(b"bench-session");
        h.update(&collective.seed_collective);
        h.finalize().into()
    };

    match mode {
        "vdf" => {
            let start = Instant::now();
            let _out = vdf::evaluate_and_generate_proof(&seed, t).expect("VDF failed");
            let elapsed = start.elapsed().as_millis();
            println!("{}", elapsed);
        }
        "zk" => {
            // ZK proving: call the full pipeline which includes ZK step
            let start = Instant::now();
            let output = crypto_engine::run_randomness_pipeline_full(
                "bench-zk", b"seed", t, 1, &[0u8; 32],
            ).expect("Pipeline failed");
            let _elapsed_total = start.elapsed().as_millis();
            // Print just ZK time from the pipeline's own measurement
            println!("{}", output.metadata.benchmark.t3_5_zkprove_ms);
        }
        _ => eprintln!("Unknown mode: {}", mode),
    }
}
RUSTEOF

echo "Building bench_offchain binary..."
(cd "$PROJECT_ROOT/off-chain" && cargo build --release --bin bench_offchain 2>&1 | tail -1)
BENCH_BIN="$PROJECT_ROOT/off-chain/target/release/bench_offchain"

if [ ! -f "$BENCH_BIN" ]; then
    echo "ERROR: bench_offchain binary not found at $BENCH_BIN"
    exit 1
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Phase 1: Measuring ZK Proving Time (constant)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
ZK_TIME_MS=0
ZK_RSS_KB=0
ZK_CPU=0

if [ -n "$GNU_TIME" ]; then
    TIME_LOG=$(mktemp /tmp/zk_time_XXXXXX.log)
    $GNU_TIME -v "$BENCH_BIN" 65536 zk 2>"$TIME_LOG" | tail -1 | read ZK_TIME_MS || ZK_TIME_MS=$("$BENCH_BIN" 65536 zk 2>/dev/null | tail -1)
    ZK_RSS_KB=$(grep "Maximum resident" "$TIME_LOG" 2>/dev/null | awk '{print $NF}' || echo "0")
    ZK_CPU=$(grep "Percent of CPU" "$TIME_LOG" 2>/dev/null | sed 's/[^0-9]//g' || echo "0")
    rm -f "$TIME_LOG"
else
    ZK_TIME_MS=$("$BENCH_BIN" 65536 zk 2>/dev/null | tail -1)
fi
echo "  ZK Proving Time: ${ZK_TIME_MS}ms | Peak RSS: ${ZK_RSS_KB}KB | CPU: ${ZK_CPU}%"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Phase 2: VDF T-Parameter Sweep"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

for EXP in "${T_EXPONENTS[@]}"; do
    T=$((1 << EXP))
    echo -n "  T=2^${EXP} (${T}): "

    VDF_TIME_MS=0
    PEAK_RSS=0
    CPU_PCT=0

    if [ -n "$GNU_TIME" ]; then
        TIME_LOG=$(mktemp /tmp/vdf_time_XXXXXX.log)
        VDF_TIME_MS=$($GNU_TIME -v "$BENCH_BIN" "$T" vdf 2>"$TIME_LOG" | tail -1)
        PEAK_RSS=$(grep "Maximum resident" "$TIME_LOG" 2>/dev/null | awk '{print $NF}' || echo "0")
        CPU_PCT=$(grep "Percent of CPU" "$TIME_LOG" 2>/dev/null | sed 's/[^0-9]//g' || echo "0")
        rm -f "$TIME_LOG"
    else
        VDF_TIME_MS=$("$BENCH_BIN" "$T" vdf 2>/dev/null | tail -1)
    fi

    echo "VDF=${VDF_TIME_MS}ms | ZK=${ZK_TIME_MS}ms | RSS=${PEAK_RSS}KB | CPU=${CPU_PCT}%"
    echo "${T},${EXP},${VDF_TIME_MS},${ZK_TIME_MS},${PEAK_RSS},${CPU_PCT}" >> "$CSV_FILE"
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  ✅ Output: $CSV_FILE"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
