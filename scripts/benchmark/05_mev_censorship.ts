import { ethers } from "hardhat";
import * as fs from "fs";
import * as path from "path";

async function main() {
  const dataDir = path.resolve(__dirname, "../../../scripts/benchmark/data");
  fs.mkdirSync(dataDir, { recursive: true });
  const csvPath = path.join(dataDir, "mev_censorship.csv");
  const rows: string[] = ["block_number,base_fee_gwei,challenge_window_sec,spam_active"];

  console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
  console.log("  Scenario 5: MEV Censorship & Dynamic Challenge Window");
  console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

  const [owner] = await ethers.getSigners();

  
  console.log("\n[1/3] Deploying contracts...");

  const VerifierFactory = await ethers.getContractFactory("Halo2Verifier");
  const zkVerifier = await VerifierFactory.deploy();
  await zkVerifier.waitForDeployment();

  const ReceiverFactory = await ethers.getContractFactory("RandomReceiver");
  const receiver = await ReceiverFactory.deploy(owner.address);
  await receiver.waitForDeployment();
  await receiver.setZkVerifier(await zkVerifier.getAddress());

  
  const thresholdWei = ethers.parseUnits("100", "gwei"); 
  await receiver.setDynamicWindowConfig(thresholdWei, 3, true);
  console.log("  Dynamic window enabled: threshold=100 Gwei, maxExpansion=3x");

  
  const TOTAL_BLOCKS = 50;
  const SPAM_START = 10;
  const SPAM_END = 30;
  const BASE_CHALLENGE_WINDOW = 600; 

  
  function getSimulatedBaseFeeGwei(blockIndex: number): number {
    if (blockIndex < SPAM_START) return 20 + Math.random() * 10; 
    if (blockIndex < SPAM_START + 3) return 100 + (blockIndex - SPAM_START) * 80; 
    if (blockIndex < SPAM_END - 3) return 500 + Math.random() * 200; 
    if (blockIndex < SPAM_END) return 300 - (blockIndex - (SPAM_END - 3)) * 60; 
    
    const decay = Math.max(20, 150 - (blockIndex - SPAM_END) * 8);
    return decay + Math.random() * 5;
  }

  console.log(`\n[2/3] Simulating ${TOTAL_BLOCKS} blocks (spam: blocks ${SPAM_START}-${SPAM_END})...\n`);

  for (let i = 0; i < TOTAL_BLOCKS; i++) {
    const spamActive = i >= SPAM_START && i < SPAM_END;
    const baseFeeGwei = getSimulatedBaseFeeGwei(i);
    const baseFeeWei = ethers.parseUnits(Math.floor(baseFeeGwei).toString(), "gwei");

    
    await ethers.provider.send("hardhat_setNextBlockBaseFeePerGas", [
      "0x" + baseFeeWei.toString(16)
    ]);

    
    const tx = await owner.sendTransaction({
      to: owner.address,
      value: 0,
      maxFeePerGas: baseFeeWei * 2n,
      maxPriorityFeePerGas: ethers.parseUnits("1", "gwei"),
    });
    await tx.wait();

    
    const block = await ethers.provider.getBlock("latest");
    const actualBaseFeeWei = block?.baseFeePerGas || 0n;
    const actualBaseFeeGwei = Number(ethers.formatUnits(actualBaseFeeWei, "gwei"));

    
    let dynamicWindow = BASE_CHALLENGE_WINDOW;
    if (actualBaseFeeWei > thresholdWei) {
      const ratio = Number(actualBaseFeeWei / thresholdWei);
      dynamicWindow = BASE_CHALLENGE_WINDOW * Math.min(ratio, 3);
    }

    const status = spamActive ? "SPAM" : "NORMAL";
    console.log(
      `  Block ${String(block?.number).padStart(5)}: ` +
      `BaseFee=${actualBaseFeeGwei.toFixed(1).padStart(7)} Gwei | ` +
      `Window=${dynamicWindow}s | ` +
      `${status}`
    );

    rows.push(`${block?.number},${actualBaseFeeGwei.toFixed(2)},${dynamicWindow},${spamActive}`);
  }

  
  fs.writeFileSync(csvPath, rows.join("\n") + "\n");
  console.log(`\n  ✅ Output: ${csvPath}`);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
