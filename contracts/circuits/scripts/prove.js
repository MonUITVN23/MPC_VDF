

const crypto = require("crypto");
const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");

const BUILD_DIR = path.join(__dirname, "..", "build");


function splitHash256(hashBuffer) {
  const hi = BigInt("0x" + hashBuffer.subarray(0, 16).toString("hex"));
  const lo = BigInt("0x" + hashBuffer.subarray(16, 32).toString("hex"));
  return { hi: hi.toString(), lo: lo.toString() };
}


function sha256(data) {
  return crypto.createHash("sha256").update(data).digest();
}


function generateMockInput() {
  const pk = crypto.randomBytes(48);
  const sig = crypto.randomBytes(96);
  const msg = crypto.randomBytes(32);
  const requestId = 42n;
  const y = crypto.randomBytes(32);
  const pi = crypto.randomBytes(32);
  const modulus = crypto.randomBytes(32);

  return { pk, sig, msg, requestId, y, pi, modulus };
}


function preparePublicSignals({ pk, sig, msg, requestId, y, pi, modulus }) {
  
  const pkPadded = Buffer.alloc(48);
  pk.copy(pkPadded, 0, 0, Math.min(pk.length, 48));

  const sigPadded = Buffer.alloc(96);
  sig.copy(sigPadded, 0, 0, Math.min(sig.length, 96));

  const msgPadded = Buffer.alloc(32);
  msg.copy(msgPadded, 0, 0, Math.min(msg.length, 32));

  
  const commitmentInput = Buffer.concat([pkPadded, sigPadded, msgPadded]);
  const commitmentHash = sha256(commitmentInput);
  const commitment = splitHash256(commitmentHash);

  
  const pkHash = sha256(pkPadded);
  const pkHashSplit = splitHash256(pkHash);

  
  const requestIdBuf = Buffer.alloc(32);
  requestIdBuf.writeBigUInt64BE(BigInt(requestId), 24);
  const payloadInput = Buffer.concat([requestIdBuf, y, pi, msgPadded, modulus]);
  const payloadHash = sha256(payloadInput);
  const payloadHashSplit = splitHash256(payloadHash);

  return {
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

  let rawInput;
  const inputIdx = args.indexOf("--input");
  if (inputIdx >= 0 && args[inputIdx + 1]) {
    rawInput = JSON.parse(fs.readFileSync(args[inputIdx + 1], "utf8"));
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

  let outputDir = BUILD_DIR;
  const outIdx = args.indexOf("--output");
  if (outIdx >= 0 && args[outIdx + 1]) {
    outputDir = args[outIdx + 1];
    fs.mkdirSync(outputDir, { recursive: true });
  }

  console.log("=== BLS Commitment ZK Prover (Halo2/KZG — No Trusted Setup) ===\n");

  
  const publicSignals = preparePublicSignals(rawInput);

  console.log("Public signals computed:");
  const signalNames = [
    "commitment_hi", "commitment_lo",
    "pk_hash_hi", "pk_hash_lo",
    "payload_hash_hi", "payload_hash_lo",
    "request_id"
  ];
  const signals = [
    publicSignals.commitment_hi, publicSignals.commitment_lo,
    publicSignals.pk_hash_hi, publicSignals.pk_hash_lo,
    publicSignals.payload_hash_hi, publicSignals.payload_hash_lo,
    publicSignals.request_id,
  ];
  signals.forEach((s, i) => {
    console.log(`  [${i}] ${signalNames[i]}: ${s}`);
  });

  
  const publicPath = path.join(outputDir, "public.json");
  fs.mkdirSync(outputDir, { recursive: true });
  fs.writeFileSync(publicPath, JSON.stringify(signals, null, 2));

  console.log(`\nPublic signals written to: ${publicPath}`);
  console.log(`\nNOTE: For full Halo2 proof generation, use the Rust halo2_prover crate:`);
  console.log(`  cd off-chain && cargo run -p halo2_prover`);
  console.log(`\nHalo2/KZG does NOT require a trusted setup ceremony.`);

  return { publicSignals: signals, provingTimeMs: 0 };
}


module.exports = { preparePublicSignals, splitHash256, sha256, generateMockInput };


if (require.main === module) {
  main().then(() => process.exit(0)).catch((e) => {
    console.error("Failed:", e);
    process.exit(1);
  });
}
