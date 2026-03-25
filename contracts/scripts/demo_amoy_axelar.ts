import { ethers } from "hardhat";
import { execSync } from "child_process";
import * as fs from "fs";
import * as path from "path";
async function withTimeout<T>(promise: Promise<T>, ms: number, message: string): Promise<T> {
  let timer: NodeJS.Timeout;
  const timeoutPromise = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error(`Timeout: ${message} after ${ms}ms`)), ms);
  });
  return Promise.race([promise, timeoutPromise]).finally(() => clearTimeout(timer));
}

async function main() {
  console.log("=== AMOY TESTNET ZK-VERIFICATION DEMO ===");
  const [deployer] = await ethers.getSigners();
  console.log(`Deployer address: ${deployer.address}`);
  
  const balance = await ethers.provider.getBalance(deployer.address);
  console.log(`Balance: ${ethers.formatEther(balance)} MATIC`);
  
  if (balance === 0n) {
    throw new Error("Deployer has no MATIC on Amoy. Please fund the account.");
  }

  // 1. Deploy Verifier
  console.log("\n1. Deploying Groth16Verifier on Amoy...");
  const VerifierFactory = await ethers.getContractFactory("Groth16Verifier");
  const verifier = await VerifierFactory.deploy();
  await verifier.waitForDeployment();
  const verifierAddress = await verifier.getAddress();
  console.log(`   -> Groth16Verifier deployed at: ${verifierAddress}`);

  // 2. Deploy Receiver
  console.log("\n2. Deploying RandomReceiver on Amoy...");
  // Use deployer as dummy gateway address for this simulation
  const ReceiverFactory = await ethers.getContractFactory("RandomReceiver");
  const receiver = await ReceiverFactory.deploy(deployer.address);
  await receiver.waitForDeployment();
  const receiverAddress = await receiver.getAddress();
  console.log(`   -> RandomReceiver deployed at: ${receiverAddress}`);

  // 3. Configure Receiver
  console.log("\n3. Configuring RandomReceiver...");
  const tx1 = await withTimeout(receiver.setZkVerifier(verifierAddress), 60000, "setZkVerifier");
  await withTimeout(tx1.wait(), 60000, "tx1.wait()");
  console.log(`   -> setZkVerifier transaction confirmed`);

  const tx2 = await withTimeout(receiver.setZkProofMode(true), 60000, "setZkProofMode");
  await withTimeout(tx2.wait(), 60000, "tx2.wait()");
  console.log(`   -> setZkProofMode(true) transaction confirmed`);

  // 4. Generate ZK Proof
  console.log("\n4. Generating ZK Proof locally using snarkJS...");
  
  const pk = new Uint8Array(48).fill(1);
  const sig = new Uint8Array(96).fill(2);
  const msg = new Uint8Array(32).fill(3);
  const y = new Uint8Array(128).fill(4);
  const pi = new Uint8Array(128).fill(5);
  const modulus = new Uint8Array(32).fill(6);
  const requestId = 10001n;

  const inputObj = {
    pk: Buffer.from(pk).toString('hex'),
    sig: Buffer.from(sig).toString('hex'),
    msg: Buffer.from(msg).toString('hex'),
    y: Buffer.from(y).toString('hex'),
    pi: Buffer.from(pi).toString('hex'),
    modulus: Buffer.from(modulus).toString('hex'),
    requestId: requestId.toString()
  };

  const tempDir = path.join(__dirname, `../circuits/temp_amoy_${Date.now()}`);
  fs.mkdirSync(tempDir, { recursive: true });
  
  const inputPath = path.join(tempDir, "input.json");
  fs.writeFileSync(inputPath, JSON.stringify(inputObj));

  const proveScript = path.join(__dirname, "../circuits/scripts/prove.js");
  execSync(`node ${proveScript} --input ${inputPath} --output ${tempDir}`, { stdio: 'inherit' });

  const proofStr = fs.readFileSync(path.join(tempDir, "proof.json"), "utf8");
  const publicStr = fs.readFileSync(path.join(tempDir, "public.json"), "utf8");
  
  const proof = JSON.parse(proofStr);
  const pubSignals = JSON.parse(publicStr);

  const zkPublicSignals = pubSignals;

  const AbiCoder = ethers.AbiCoder.defaultAbiCoder();
  const zkProofData = AbiCoder.encode(
    ["uint256[2]", "uint256[2][2]", "uint256[2]"],
    [
      [proof.pi_a[0], proof.pi_a[1]],
      [
        [proof.pi_b[0][1], proof.pi_b[0][0]],
        [proof.pi_b[1][1], proof.pi_b[1][0]]
      ],
      [proof.pi_c[0], proof.pi_c[1]]
    ]
  );

  const expectedPkHash = ethers.zeroPadValue("0x" + BigInt(pubSignals[2]).toString(16), 32);
  const tx3 = await withTimeout(receiver.registerPkHash(expectedPkHash), 60000, "registerPkHash");
  await withTimeout(tx3.wait(), 60000, "tx3.wait()");
  console.log(`   -> registerPkHash transaction confirmed`);

  fs.rmSync(tempDir, { recursive: true, force: true });

  // 5. Submit to Amoy
  console.log("\n5. Submitting ZK Payload to Amoy (Simulating Axelar Bridge Delivery)...");
  try {
    const submitTx = await withTimeout(
      receiver.submitOptimisticResult(
        requestId,
        y,
        pi,
        msg,
        modulus,
        sig,
        zkProofData,
        zkPublicSignals
      ),
      60000,
      "submitOptimisticResult RPC call"
    );
    
    console.log(`   -> Transaction sent! Hash: ${submitTx.hash}`);
    console.log(`   -> Waiting for block confirmation...`);
    const receipt = await withTimeout(submitTx.wait(), 60000, "Transaction confirmation");
    console.log(`   -> SUCCESS! Transaction confirmed in block ${receipt?.blockNumber}`);
    console.log(`   -> Gas Used on Amoy: ${receipt?.gasUsed?.toString()}`);
    console.log(`\nView on Explorer: https://amoy.polygonscan.com/tx/${submitTx.hash}`);

  } catch (err: any) {
    console.error("   -> Transaction failed:", err.message);
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
