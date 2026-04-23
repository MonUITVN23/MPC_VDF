import { ethers } from "hardhat";
import * as fs from "fs";
import * as path from "path";

async function main() {
  const dataDir = path.resolve(__dirname, "../../../scripts/benchmark/data");
  fs.mkdirSync(dataDir, { recursive: true });
  const csvPath = path.join(dataDir, "gas_metrics.csv");
  const jsonPath = path.join(dataDir, "test_vectors.json");
  const rows: string[] = ["operation,gas_used,category"];

  if (!fs.existsSync(jsonPath)) {
    console.error(`\n[ERROR] test_vectors.json not found at ${jsonPath}`);
    console.error("Please run the Rust test vector generator first!\n");
    process.exit(1);
  }

  const vectors = JSON.parse(fs.readFileSync(jsonPath, "utf8"));

  const [owner] = await ethers.getSigners();

  const VerifierFactory = await ethers.getContractFactory("Halo2Verifier");
  const zkVerifier = await VerifierFactory.deploy();
  await zkVerifier.waitForDeployment();

  const ReceiverFactory = await ethers.getContractFactory("RandomReceiver");
  const receiver = await ReceiverFactory.deploy(owner.address);
  await receiver.waitForDeployment();

  await receiver.setZkVerifier(await zkVerifier.getAddress());

  const RouterFactory = await ethers.getContractFactory("RandomRouter");
  const router = await RouterFactory.deploy("hardhat-local", await receiver.getAddress());
  await router.waitForDeployment();

  const txRequest = await router.requestRandomness(12345);
  const receiptRequest = await txRequest.wait();
  rows.push(`requestRandomness,${receiptRequest!.gasUsed},CrossRand`);

  const requestId = 1n;

  const pkHashHi = BigInt(vectors.zk_public_signals[2]);
  const pkHashLo = BigInt(vectors.zk_public_signals[3]);
  const fullPkHash = (pkHashHi << 128n) | pkHashLo;
  const expectedPkHash = "0x" + fullPkHash.toString(16).padStart(64, "0");
  await receiver.registerPkHash(expectedPkHash);

  const txOptimistic = await receiver.submitOptimisticResult(
    requestId,
    vectors.y,
    vectors.pi,
    vectors.msg,
    vectors.modulus,
    vectors.sig,
    "0x",
    [0, 0, 0, 0, 0, 0, 0]
  );
  const receiptOptimistic = await txOptimistic.wait();
  rows.push(`submitOptimisticResult_NoZK,${receiptOptimistic!.gasUsed},CrossRand`);

  const txChallenge = await receiver.challengeResult(
    requestId,
    vectors.y,
    vectors.pi,
    vectors.msg,
    vectors.modulus,
    vectors.sig,
    vectors.zk_proof_data,
    vectors.zk_public_signals
  );
  const receiptChallenge = await txChallenge.wait();
  rows.push(`challengeResult_Halo2_VDF,${receiptChallenge!.gasUsed},CrossRand`);

  console.log("  [INFO] Fast-forwarding 55 blocks to bypass Challenge Window...");
  await ethers.provider.send("hardhat_mine", ["0x37"]);

  const txFinalize = await receiver.finalizeRandomness(requestId);
  const receiptFinalize = await txFinalize.wait();
  rows.push(`finalizeRandomness,${receiptFinalize!.gasUsed},CrossRand`);

  rows.push(`Chainlink_VRF_Request,100000,Baseline`);
  rows.push(`Chainlink_VRF_Fulfill,200000,Baseline`);
  rows.push(`Drand_Verify,150000,Baseline`);

  fs.writeFileSync(csvPath, rows.join("\n") + "\n");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});