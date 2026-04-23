#!/bin/bash
set -e

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
PROJECT_ROOT="$DIR/../.."
if [ -f "$PROJECT_ROOT/.env" ]; then
    export $(grep -v '^#' "$PROJECT_ROOT/.env" | xargs)
fi
DATA_DIR="$DIR/data"
mkdir -p "$DATA_DIR"

echo "Building offchain benchmark binary..."
cd "$PROJECT_ROOT/off-chain/crypto_engine"
cargo build --release --bin bench_offchain
BIN_PATH="../target/release/bench_offchain"

VDF_CSV="$DATA_DIR/bench_vdf.csv"
ZK_CSV="$DATA_DIR/bench_zk.csv"

echo "=== PHASE 1: VDF Sweep ==="
echo "t,vdf_ms" > "$VDF_CSV"
for i in {16..21}; do
    T=$((2**i))
    echo -n "Measuring VDF for T=$T... "
    ELAPSED=$($BIN_PATH $T vdf)
    echo "Done in $ELAPSED ms"
    echo "$T,$ELAPSED" >> "$VDF_CSV"
done

echo "=== PHASE 2: ZK Profiling ==="
echo "run_id,zk_ms,peak_rss_kb" > "$ZK_CSV"
for i in {1..5}; do
    echo -n "Measuring ZK for run=$i... "
    TMP_STDOUT=$(mktemp)
    TMP_STDERR=$(mktemp)
    
    /usr/bin/time -v $BIN_PATH 10 zk > "$TMP_STDOUT" 2> "$TMP_STDERR"
    
    ZK_MS=$(cat "$TMP_STDOUT")
    PEAK_RSS=$(grep "Maximum resident set size" "$TMP_STDERR" | awk '{print $6}')
    
    echo "zk_ms=$ZK_MS, peak_rss=${PEAK_RSS}KB"
    echo "$i,$ZK_MS,$PEAK_RSS" >> "$ZK_CSV"
    
    rm -f "$TMP_STDOUT" "$TMP_STDERR"
done
