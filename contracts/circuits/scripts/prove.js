/**
 * prove.js — Generate a Groth16 ZK-SNARK proof for the BLS Commitment circuit.
 *
 * This script:
 *   1. Takes input data (pk, sig, msg, payload_hash, pk_hash, request_id)
 *   2. Computes SHA256 commitments
 *   3. Generates a Groth16 proof using snarkjs
 *   4. Outputs proof.json and public.json
 *
 * Usage:
 *   node scripts/prove.js [--input <path>] [--output <dir>]
 *   node scripts/prove.js --demo   # Generate demo proof with mock data
 */

const snarkjs = require("snarkjs");
const crypto = require("crypto");
const fs = require("fs");
const path = require("path");

const BUILD_DIR = path.join(__dirname, "..", "build");
const WASM_PATH = path.join(BUILD_DIR, "bls_commitment_js", "bls_commitment.wasm");
const ZKEY_PATH = path.join(BUILD_DIR, "bls_commitment.zkey");

/**
 * Split a 256-bit hash (Buffer) into two 128-bit BigInt values (hi, lo)
 * matching the circuit's Bits2Num128x2 output.
 */
function splitHash256(hashBuffer) {
  const hi = BigInt("0x" + hashBuffer.subarray(0, 16).toString("hex"));
  const lo = BigInt("0x" + hashBuffer.subarray(16, 32).toString("hex"));
  return { hi: hi.toString(), lo: lo.toString() };
}

/**
 * Compute SHA-256 hash and return Buffer (32 bytes)
 */
function sha256(data) {
  return crypto.createHash("sha256").update(data).digest();
}

/**
 * Generate mock BLS12-381 data for demo/testing.
 * In production, this comes from the MPC/VDF pipeline.
 */
function generateMockInput() {
  // Mock compressed public key (48 bytes)
  const pk = crypto.randomBytes(48);
  // Mock compressed signature (96 bytes)
  const sig = crypto.randomBytes(96);
  // Mock message / seed_collective (32 bytes)
  const msg = crypto.randomBytes(32);

  // Mock payload components
  const requestId = 42n;
  const y = crypto.randomBytes(32);
  const pi = crypto.randomBytes(32);
  const modulus = crypto.randomBytes(32);

  return { pk, sig, msg, requestId, y, pi, modulus };
}

/**
 * Prepare circuit inputs from raw data.
 */
function prepareCircuitInput({ pk, sig, msg, requestId, y, pi, modulus }) {
  // Compute commitment = SHA256(pk || sig || msg)
  const commitmentInput = Buffer.concat([pk, sig, msg]);
  const commitmentHash = sha256(commitmentInput);
  const commitment = splitHash256(commitmentHash);

  // Compute pk_hash = SHA256(pk)
  const pkHash = sha256(pk);
  const pkHashSplit = splitHash256(pkHash);

  // Compute payload_hash = SHA256(requestId_be32 || y || pi || msg || modulus)
  const requestIdBuf = Buffer.alloc(32);
  requestIdBuf.writeBigUInt64BE(BigInt(requestId), 24); // right-aligned in 32 bytes
  const payloadInput = Buffer.concat([requestIdBuf, y, pi, msg, modulus]);
  const payloadHash = sha256(payloadInput);
  const payloadHashSplit = splitHash256(payloadHash);

  return {
    // Private inputs (byte arrays)
    pk: Array.from(pk).map(String),
    sig: Array.from(sig).map(String),
    msg: Array.from(msg).map(String),

    // Public inputs
    commitment_hi: commitment.hi,
    commitment_lo: commitment.lo,
    pk_hash_hi: pkHashSplit.hi,
    pk_hash_lo: pkHashSplit.lo,
    payload_hash_hi: payloadHashSplit.hi,
    payload_hash_lo: payloadHashSplit.lo,
    request_id: requestId.toString(),
  };
}

async function main() {
  const args = process.argv.slice(2);
  const isDemo = args.includes("--demo");

  let rawInput;
  const inputIdx = args.indexOf("--input");
  if (inputIdx >= 0 && args[inputIdx + 1]) {
    rawInput = JSON.parse(fs.readFileSync(args[inputIdx + 1], "utf8"));
    // Convert hex strings to Buffers
    rawInput.pk = Buffer.from(rawInput.pk, "hex");
    rawInput.sig = Buffer.from(rawInput.sig, "hex");
    rawInput.msg = Buffer.from(rawInput.msg, "hex");
    rawInput.y = Buffer.from(rawInput.y, "hex");
    rawInput.pi = Buffer.from(rawInput.pi, "hex");
    rawInput.modulus = Buffer.from(rawInput.modulus, "hex");
    rawInput.requestId = BigInt(rawInput.requestId);
  } else {
    console.log("Using mock data for demo...\n");
    rawInput = generateMockInput();
  }

  // Determine output directory
  let outputDir = BUILD_DIR;
  const outIdx = args.indexOf("--output");
  if (outIdx >= 0 && args[outIdx + 1]) {
    outputDir = args[outIdx + 1];
    fs.mkdirSync(outputDir, { recursive: true });
  }

  console.log("=== BLS Commitment ZK Prover ===\n");

  // Prepare circuit input
  const circuitInput = prepareCircuitInput(rawInput);

  // Save input for debugging
  const inputPath = path.join(outputDir, "input.json");
  fs.writeFileSync(inputPath, JSON.stringify(circuitInput, null, 2));
  console.log(`Input written to: ${inputPath}`);

  // Check build files exist
  if (!fs.existsSync(WASM_PATH)) {
    console.error(`ERROR: WASM not found at ${WASM_PATH}. Run 'bash scripts/setup.sh' first.`);
    process.exit(1);
  }
  if (!fs.existsSync(ZKEY_PATH)) {
    console.error(`ERROR: zkey not found at ${ZKEY_PATH}. Run 'bash scripts/setup.sh' first.`);
    process.exit(1);
  }

  // Generate proof
  console.log("\nGenerating Groth16 proof...");
  const startTime = Date.now();

  const { proof, publicSignals } = await snarkjs.groth16.fullProve(
    circuitInput,
    WASM_PATH,
    ZKEY_PATH
  );

  const provingTimeMs = Date.now() - startTime;

  // Save outputs
  const proofPath = path.join(outputDir, "proof.json");
  const publicPath = path.join(outputDir, "public.json");

  fs.writeFileSync(proofPath, JSON.stringify(proof, null, 2));
  fs.writeFileSync(publicPath, JSON.stringify(publicSignals, null, 2));

  console.log(`\nProof generated successfully!`);
  console.log(`  Proving time: ${provingTimeMs} ms`);
  console.log(`  Proof:        ${proofPath}`);
  console.log(`  Public:       ${publicPath}`);
  console.log(`  Public signals (${publicSignals.length}):`);
  const signalNames = [
    "commitment_hi", "commitment_lo",
    "pk_hash_hi", "pk_hash_lo",
    "payload_hash_hi", "payload_hash_lo",
    "request_id"
  ];
  publicSignals.forEach((s, i) => {
    const name = signalNames[i] || `signal_${i}`;
    console.log(`    [${i}] ${name}: ${s}`);
  });

  // Return structure for programmatic use
  return { proof, publicSignals, provingTimeMs };
}

// Export for use as module
module.exports = { prepareCircuitInput, splitHash256, sha256, generateMockInput };

// Run if called directly
if (require.main === module) {
  main().catch((e) => {
    console.error("Proving failed:", e);
    process.exit(1);
  });
}
