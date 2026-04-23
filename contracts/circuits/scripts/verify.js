

const fs = require("fs");
const path = require("path");

const BUILD_DIR = path.join(__dirname, "..", "build");

async function main() {
  const args = process.argv.slice(2);

  let publicPath = path.join(BUILD_DIR, "public.json");
  const pubIdx = args.indexOf("--public");
  if (pubIdx >= 0 && args[pubIdx + 1]) publicPath = args[pubIdx + 1];

  console.log("=== BLS Commitment ZK Verifier (Halo2/KZG) ===\n");

  if (!fs.existsSync(publicPath)) {
    console.error(`ERROR: Public signals not found at ${publicPath}`);
    process.exit(1);
  }

  const publicSignals = JSON.parse(fs.readFileSync(publicPath, "utf8"));

  console.log(`Public signals: ${publicPath}`);
  console.log(`Public signals count: ${publicSignals.length}\n`);

  if (publicSignals.length !== 7) {
    console.error(`ERROR: Expected 7 public signals, got ${publicSignals.length}`);
    process.exit(1);
  }

  const signalNames = [
    "commitment_hi", "commitment_lo",
    "pk_hash_hi", "pk_hash_lo",
    "payload_hash_hi", "payload_hash_lo",
    "request_id"
  ];

  publicSignals.forEach((s, i) => {
    console.log(`  [${i}] ${signalNames[i]}: ${s}`);
  });

  console.log(`\n✅ Public signals are valid (7 fields present)`);
  console.log(`\nNOTE: Full proof verification is done by:`);
  console.log(`  - On-chain: Halo2Verifier.sol (EVM pairing precompiles)`);
  console.log(`  - Off-chain: halo2_prover::verify() (Rust)`);
  console.log(`\nHalo2/KZG requires NO trusted setup ceremony.`);

  return { isValid: true };
}

if (require.main === module) {
  main().catch((e) => {
    console.error("Verification failed:", e);
    process.exit(1);
  });
}
