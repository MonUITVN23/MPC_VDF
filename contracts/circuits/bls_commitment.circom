pragma circom 2.0.0;

include "../node_modules/circomlib/circuits/sha256/sha256.circom";
include "../node_modules/circomlib/circuits/bitify.circom";
include "../node_modules/circomlib/circuits/comparators.circom";

/*
 * BLS Signature Commitment Circuit (Tier 1)
 *
 * Purpose:
 *   Prove knowledge of (pk, sig, msg) that hash to a known commitment,
 *   while binding the proof to a specific payload_hash, pk_hash, and request_id.
 *
 * Security model:
 *   - The BLS12-381 signature is verified off-chain by the trusted host before proving.
 *   - This circuit proves: "I know (pk, sig, msg) such that SHA256(pk || sig || msg) = commitment"
 *   - payload_hash and pk_hash are passed as public inputs and checked for consistency.
 *   - On-chain verifier checks: commitment, payload_hash, pk_hash, request_id all match.
 *
 * Input sizes (BLS12-381):
 *   - pk:  48 bytes (compressed G1) = 384 bits
 *   - sig: 96 bytes (compressed G2) = 768 bits
 *   - msg: 32 bytes (seed_collective) = 256 bits
 *   Total private input to SHA256: 48 + 96 + 32 = 176 bytes = 1408 bits
 *
 * Constraints estimate:
 *   - SHA256 in circomlib: ~29,000 constraints per 512-bit block
 *   - 1408 bits + 64 bits padding + length => 3 blocks => ~87,000 constraints
 *   - Plus equality checks => total ~100,000 constraints
 *   - Fits comfortably in ptau 2^17 (131,072) or 2^18 (262,144)
 */

// ---------------------------------------------------------------
// Helper: Convert N bytes (as array of integers 0-255) to bits
// ---------------------------------------------------------------
template BytesToBits(nBytes) {
    signal input in[nBytes];
    signal output out[nBytes * 8];

    component n2b[nBytes];
    for (var i = 0; i < nBytes; i++) {
        n2b[i] = Num2Bits(8);
        n2b[i].in <== in[i];
        // Num2Bits outputs LSB first, SHA256 expects MSB first
        for (var j = 0; j < 8; j++) {
            out[i * 8 + j] <== n2b[i].out[7 - j];
        }
    }
}

// ---------------------------------------------------------------
// Helper: Convert 256 bits to 2 x 128-bit field elements
// (Solidity-friendly: fits in uint256 as two halves)
// ---------------------------------------------------------------
template Bits2Num128x2() {
    signal input in[256];
    signal output hi;
    signal output lo;

    component b2n_hi = Bits2Num(128);
    component b2n_lo = Bits2Num(128);

    for (var i = 0; i < 128; i++) {
        b2n_hi.in[127 - i] <== in[i];        // bits 0..127  => hi
        b2n_lo.in[127 - i] <== in[128 + i];  // bits 128..255 => lo
    }

    hi <== b2n_hi.out;
    lo <== b2n_lo.out;
}

// ---------------------------------------------------------------
// Main Component: BLS Commitment Verifier
// ---------------------------------------------------------------
template BlsCommitment() {
    // === Private inputs ===
    // BLS12-381 public key (compressed G1): 48 bytes
    signal input pk[48];
    // BLS12-381 aggregate signature (compressed G2): 96 bytes
    signal input sig[96];
    // Message (seed_collective): 32 bytes
    signal input msg[32];

    // === Public inputs ===
    // Expected commitment = SHA256(pk || sig || msg), split into hi/lo 128-bit parts
    signal input commitment_hi;
    signal input commitment_lo;
    // Expected pk_hash = SHA256(pk), split into hi/lo
    signal input pk_hash_hi;
    signal input pk_hash_lo;
    // payload_hash = SHA256(requestId || y || pi || seedCollective || modulus)
    // Computed off-circuit, checked on-chain for binding
    signal input payload_hash_hi;
    signal input payload_hash_lo;
    // request_id (fits in uint128 for our PoC)
    signal input request_id;

    // === Step 1: Compute SHA256(pk || sig || msg) ===
    // Total input: 48 + 96 + 32 = 176 bytes = 1408 bits
    var totalBytes = 48 + 96 + 32;  // 176
    var totalBits = totalBytes * 8;   // 1408

    component inputBits = BytesToBits(totalBytes);
    for (var i = 0; i < 48; i++) {
        inputBits.in[i] <== pk[i];
    }
    for (var i = 0; i < 96; i++) {
        inputBits.in[48 + i] <== sig[i];
    }
    for (var i = 0; i < 32; i++) {
        inputBits.in[48 + 96 + i] <== msg[i];
    }

    component sha_commitment = Sha256(totalBits);
    for (var i = 0; i < totalBits; i++) {
        sha_commitment.in[i] <== inputBits.out[i];
    }

    // Convert hash output (256 bits) to 2 x 128-bit numbers
    component commitHash = Bits2Num128x2();
    for (var i = 0; i < 256; i++) {
        commitHash.in[i] <== sha_commitment.out[i];
    }

    // === Step 2: Verify commitment matches public input ===
    commitHash.hi === commitment_hi;
    commitHash.lo === commitment_lo;

    // === Step 3: Compute SHA256(pk) and verify pk_hash ===
    var pkBits = 48 * 8;  // 384 bits

    component pkInputBits = BytesToBits(48);
    for (var i = 0; i < 48; i++) {
        pkInputBits.in[i] <== pk[i];
    }

    component sha_pk = Sha256(pkBits);
    for (var i = 0; i < pkBits; i++) {
        sha_pk.in[i] <== pkInputBits.out[i];
    }

    component pkHash = Bits2Num128x2();
    for (var i = 0; i < 256; i++) {
        pkHash.in[i] <== sha_pk.out[i];
    }

    pkHash.hi === pk_hash_hi;
    pkHash.lo === pk_hash_lo;

    // === Step 4: Constrain public signals ===
    // payload_hash and request_id are pure public inputs — they are NOT
    // derived inside the circuit. They exist to bind the proof to a specific
    // payload and request. The on-chain verifier will:
    //   1. Recompute payload_hash from actual decoded payload data
    //   2. Compare with the payload_hash committed in the proof
    //   3. Compare request_id with the actual request
    //
    // We add dummy constraints to prevent compiler from optimizing them away.
    signal payload_hash_bound;
    payload_hash_bound <== payload_hash_hi + payload_hash_lo;

    signal request_id_bound;
    request_id_bound <== request_id * 1;
}

component main {public [
    commitment_hi,
    commitment_lo,
    pk_hash_hi,
    pk_hash_lo,
    payload_hash_hi,
    payload_hash_lo,
    request_id
]} = BlsCommitment();
