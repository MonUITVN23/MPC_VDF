#!/usr/bin/env bash
# ============================================================
# Trusted Setup Script for BLS Commitment Circuit
# ============================================================
# Usage: bash scripts/setup.sh [--tier2]
#
# This script:
#   1. Compiles the circom circuit
#   2. Downloads Powers of Tau (community ceremony)
#   3. Runs Groth16 phase-2 setup
#   4. Exports verification key and Solidity verifier
#
# Requirements: circom, snarkjs, node >= 18
# ============================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CIRCUIT_DIR="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$CIRCUIT_DIR/build"
PTAU_DIR="$CIRCUIT_DIR/ptau"

# Tier selection
TIER="${1:-tier1}"
if [[ "$TIER" == "--tier2" ]]; then
    PTAU_POWER=24
    PTAU_FILE="powersOfTau28_hez_final_24.ptau"
    PTAU_URL="https://storage.googleapis.com/zkevm/ptau/powersOfTau28_hez_final_24.ptau"
    echo "=== Tier 2 Setup (ptau 2^24, ~5GB download) ==="
else
    PTAU_POWER=18
    PTAU_FILE="powersOfTau28_hez_final_18.ptau"
    PTAU_URL="https://storage.googleapis.com/zkevm/ptau/powersOfTau28_hez_final_18.ptau"
    echo "=== Tier 1 Setup (ptau 2^18, ~75MB download) ==="
fi

mkdir -p "$BUILD_DIR" "$PTAU_DIR"

# ---- Step 1: Compile Circuit ----
echo ""
echo "[1/5] Compiling circuit..."
circom "$CIRCUIT_DIR/bls_commitment.circom" \
    --r1cs \
    --wasm \
    --sym \
    --output "$BUILD_DIR" \
    -l "$CIRCUIT_DIR/node_modules"

echo "  R1CS: $BUILD_DIR/bls_commitment.r1cs"
echo "  WASM: $BUILD_DIR/bls_commitment_js/bls_commitment.wasm"

# Print circuit info
echo ""
echo "[1.5/5] Circuit info:"
npx snarkjs r1cs info "$BUILD_DIR/bls_commitment.r1cs"

# ---- Step 2: Download Powers of Tau ----
echo ""
echo "[2/5] Downloading Powers of Tau (Hermez ceremony, 2^$PTAU_POWER)..."
if [[ -f "$PTAU_DIR/$PTAU_FILE" ]]; then
    echo "  Already exists: $PTAU_DIR/$PTAU_FILE"
else
    wget -q --show-progress -O "$PTAU_DIR/$PTAU_FILE" "$PTAU_URL"
    echo "  Downloaded: $PTAU_DIR/$PTAU_FILE"
fi

# ---- Step 3: Groth16 Phase 2 Setup ----
echo ""
echo "[3/5] Groth16 phase 2 setup..."
npx snarkjs groth16 setup \
    "$BUILD_DIR/bls_commitment.r1cs" \
    "$PTAU_DIR/$PTAU_FILE" \
    "$BUILD_DIR/bls_commitment_0000.zkey"

# Contribute to phase 2 (deterministic for reproducibility)
echo "Applying phase 2 contribution..."
npx snarkjs zkey contribute \
    "$BUILD_DIR/bls_commitment_0000.zkey" \
    "$BUILD_DIR/bls_commitment.zkey" \
    --name="mpc-vdf-poc" \
    -e="mpc-vdf-zk-commitment-phase2-entropy"

rm -f "$BUILD_DIR/bls_commitment_0000.zkey"

# ---- Step 4: Export Verification Key ----
echo ""
echo "[4/5] Exporting verification key..."
npx snarkjs zkey export verificationkey \
    "$BUILD_DIR/bls_commitment.zkey" \
    "$BUILD_DIR/verification_key.json"

echo "  Verification key: $BUILD_DIR/verification_key.json"

# ---- Step 5: Export Solidity Verifier ----
echo ""
echo "[5/5] Generating Solidity verifier contract..."
VERIFIER_SOL="$CIRCUIT_DIR/../src/Groth16Verifier.sol"
npx snarkjs zkey export solidityverifier \
    "$BUILD_DIR/bls_commitment.zkey" \
    "$VERIFIER_SOL"

echo "  Solidity verifier: $VERIFIER_SOL"

echo ""
echo "============================================"
echo "  Setup complete!"
echo "  Build dir:      $BUILD_DIR"
echo "  zkey:           $BUILD_DIR/bls_commitment.zkey"
echo "  vkey:           $BUILD_DIR/verification_key.json"
echo "  Verifier.sol:   $VERIFIER_SOL"
echo "============================================"
