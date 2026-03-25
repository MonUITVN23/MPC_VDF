/**
 * verify.js — Verify a Groth16 ZK-SNARK proof locally.
 *
 * Usage:
 *   node scripts/verify.js [--proof <path>] [--public <path>]
 *   node scripts/verify.js  # defaults to build/proof.json + build/public.json
 */

const snarkjs = require("snarkjs");
const fs = require("fs");
const path = require("path");

const BUILD_DIR = path.join(__dirname, "..", "build");

async function main() {
  const args = process.argv.slice(2);

  // Parse args
  let proofPath = path.join(BUILD_DIR, "proof.json");
  let publicPath = path.join(BUILD_DIR, "public.json");
  const vkeyPath = path.join(BUILD_DIR, "verification_key.json");

  const proofIdx = args.indexOf("--proof");
  if (proofIdx >= 0 && args[proofIdx + 1]) proofPath = args[proofIdx + 1];
  const pubIdx = args.indexOf("--public");
  if (pubIdx >= 0 && args[pubIdx + 1]) publicPath = args[pubIdx + 1];

  console.log("=== BLS Commitment ZK Verifier ===\n");

  // Load files
  if (!fs.existsSync(vkeyPath)) {
    console.error(`ERROR: Verification key not found at ${vkeyPath}`);
    console.error("       Run 'bash scripts/setup.sh' first.");
    process.exit(1);
  }
  if (!fs.existsSync(proofPath)) {
    console.error(`ERROR: Proof not found at ${proofPath}`);
    console.error("       Run 'node scripts/prove.js --demo' first.");
    process.exit(1);
  }
  if (!fs.existsSync(publicPath)) {
    console.error(`ERROR: Public signals not found at ${publicPath}`);
    process.exit(1);
  }

  const vkey = JSON.parse(fs.readFileSync(vkeyPath, "utf8"));
  const proof = JSON.parse(fs.readFileSync(proofPath, "utf8"));
  const publicSignals = JSON.parse(fs.readFileSync(publicPath, "utf8"));

  console.log(`Proof:          ${proofPath}`);
  console.log(`Public signals: ${publicPath}`);
  console.log(`Verification key: ${vkeyPath}`);
  console.log(`Public signals count: ${publicSignals.length}\n`);

  // Verify
  const startTime = Date.now();
  const isValid = await snarkjs.groth16.verify(vkey, publicSignals, proof);
  const verifyTimeMs = Date.now() - startTime;

  if (isValid) {
    console.log(`✅ Proof is VALID!`);
    console.log(`   Verification time: ${verifyTimeMs} ms`);
  } else {
    console.log(`❌ Proof is INVALID!`);
    process.exit(1);
  }

  // Print calldata for on-chain verification
  console.log("\n--- Solidity Calldata ---");
  const calldata = await snarkjs.groth16.exportSolidityCallData(proof, publicSignals);
  console.log(calldata);

  return { isValid, verifyTimeMs };
}

if (require.main === module) {
  main().catch((e) => {
    console.error("Verification failed:", e);
    process.exit(1);
  });
}
