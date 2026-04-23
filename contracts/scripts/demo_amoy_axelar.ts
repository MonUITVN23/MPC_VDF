import { ethers } from "hardhat";
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

  
  console.log("\n1. Deploying Halo2Verifier on Amoy...");
  const VerifierFactory = await ethers.getContractFactory("Halo2Verifier");
  const verifier = await VerifierFactory.deploy();
  await verifier.waitForDeployment();
  const verifierAddress = await verifier.getAddress();
  console.log(`   -> Halo2Verifier deployed at: ${verifierAddress}`);

  
  console.log("\n2. Deploying RandomReceiver on Amoy...");
  
  const ReceiverFactory = await ethers.getContractFactory("RandomReceiver");
  const receiver = await ReceiverFactory.deploy(deployer.address);
  await receiver.waitForDeployment();
  const receiverAddress = await receiver.getAddress();
  console.log(`   -> RandomReceiver deployed at: ${receiverAddress}`);

  
  console.log("\n3. Configuring RandomReceiver...");
  const tx1 = await withTimeout(receiver.setZkVerifier(verifierAddress), 60000, "setZkVerifier");
  await withTimeout(tx1.wait(), 60000, "tx1.wait()");
  console.log(`   -> setZkVerifier transaction confirmed`);

  const tx2 = await withTimeout(receiver.setZkProofMode(true), 60000, "setZkProofMode");
  await withTimeout(tx2.wait(), 60000, "tx2.wait()");
  console.log(`   -> setZkProofMode(true) transaction confirmed`);

  
  console.log("\n4. Preparing Halo2 proof payload...");
  
  const pk = new Uint8Array(48).fill(1);
  const sig = new Uint8Array(96).fill(2);
  const msg = new Uint8Array(32).fill(3);
  const y = new Uint8Array(128).fill(4);
  const pi = new Uint8Array(128).fill(5);
  const modulus = new Uint8Array(32).fill(6);
  const requestId = 10001n;

  const proofBytes = new Uint8Array(256).fill(0xab);
  proofBytes.fill(0, 0, 64);
  proofBytes[31] = 1;
  proofBytes[63] = 2;
  proofBytes.fill(0, proofBytes.length - 64, proofBytes.length);
  proofBytes[proofBytes.length - 33] = 1;
  proofBytes[proofBytes.length - 1] = 2;

  const zkPublicSignals = [
    1n,
    2n,
    3n,
    4n,
    5n,
    6n,
    requestId,
  ];

  const AbiCoder = ethers.AbiCoder.defaultAbiCoder();
  const zkProofData = AbiCoder.encode(["bytes"], [proofBytes]);

  const pkHashHi = zkPublicSignals[2];
  const pkHashLo = zkPublicSignals[3];
  const fullPkHash = (pkHashHi << 128n) | pkHashLo;
  const expectedPkHash = "0x" + fullPkHash.toString(16).padStart(64, "0");
  const tx3 = await withTimeout(receiver.registerPkHash(expectedPkHash), 60000, "registerPkHash");
  await withTimeout(tx3.wait(), 60000, "tx3.wait()");
  console.log(`   -> registerPkHash transaction confirmed`);

  
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
