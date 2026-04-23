#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

echo "[clean] Removing benchmark outputs..."
rm -f "$ROOT_DIR/scripts/benchmark/data/"*.csv || true
rm -f "$ROOT_DIR/scripts/benchmark/data/charts/"*.png || true

echo "[clean] Removing temporary benchmark proof folders..."
rm -rf "$ROOT_DIR/contracts/scripts/benchmark/"temp_gas_* || true

echo "[clean] Removing Python cache directories..."
find "$ROOT_DIR/scripts" "$ROOT_DIR/test" -type d -name "__pycache__" -prune -exec rm -rf {} + 2>/dev/null || true
find "$ROOT_DIR/scripts" "$ROOT_DIR/test" -type f -name "*.pyc" -delete 2>/dev/null || true

echo "[clean] Benchmark workspace is now clean."
