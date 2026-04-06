// =============================================================================
// Scenario 5: MEV Censorship & Dynamic Challenge Window
// Simulates baseFee spikes via hardhat_setNextBlockBaseFeePerGas,
// tracks dynamic challenge window expansion
// Output: scripts/benchmark/data/mev_censorship.csv
// Usage: cd contracts && npx hardhat run scripts/benchmark/05_mev_censorship.ts --network hardhat
// =============================================================================
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

  // ── Deploy Contracts ──
  console.log("\n[1/3] Deploying contracts...");

  const VerifierFactory = await ethers.getContractFactory("Groth16Verifier");
  const zkVerifier = await VerifierFactory.deploy();
  await zkVerifier.waitForDeployment();

  const ReceiverFactory = await ethers.getContractFactory("RandomReceiver");
  const receiver = await ReceiverFactory.deploy(owner.address);
  await receiver.waitForDeployment();
  await receiver.setZkVerifier(await zkVerifier.getAddress());

  // Enable dynamic challenge window on contract
  const thresholdWei = ethers.parseUnits("100", "gwei"); // 100 Gwei threshold
  await receiver.setDynamicWindowConfig(thresholdWei, 3, true);
  console.log("  Dynamic window enabled: threshold=100 Gwei, maxExpansion=3x");

  // ── Simulation Parameters ──
  const TOTAL_BLOCKS = 50;
  const SPAM_START = 10;
  const SPAM_END = 30;
  const BASE_CHALLENGE_WINDOW = 600; // 10 minutes

  // BaseFee profile: ramp up during spam, peak at ~600 Gwei, then decay
  function getSimulatedBaseFeeGwei(blockIndex: number): number {
    if (blockIndex < SPAM_START) return 20 + Math.random() * 10; // Normal: 20-30 Gwei
    if (blockIndex < SPAM_START + 3) return 100 + (blockIndex - SPAM_START) * 80; // Ramp up
    if (blockIndex < SPAM_END - 3) return 500 + Math.random() * 200; // Peak spam: 500-700 Gwei
    if (blockIndex < SPAM_END) return 300 - (blockIndex - (SPAM_END - 3)) * 60; // Ramp down
    // Post-spam decay
    const decay = Math.max(20, 150 - (blockIndex - SPAM_END) * 8);
    return decay + Math.random() * 5;
  }

  console.log(`\n[2/3] Simulating ${TOTAL_BLOCKS} blocks (spam: blocks ${SPAM_START}-${SPAM_END})...\n`);

  for (let i = 0; i < TOTAL_BLOCKS; i++) {
    const spamActive = i >= SPAM_START && i < SPAM_END;
    const baseFeeGwei = getSimulatedBaseFeeGwei(i);
    const baseFeeWei = ethers.parseUnits(Math.floor(baseFeeGwei).toString(), "gwei");

    // Set next block's baseFee via Hardhat cheatcode
    await ethers.provider.send("hardhat_setNextBlockBaseFeePerGas", [
      "0x" + baseFeeWei.toString(16)
    ]);

    // Mine block with a filler tx (needed for baseFee to take effect)
    const tx = await owner.sendTransaction({
      to: owner.address,
      value: 0,
      maxFeePerGas: baseFeeWei * 2n,
      maxPriorityFeePerGas: ethers.parseUnits("1", "gwei"),
    });
    await tx.wait();

    // Read block to confirm baseFee
    const block = await ethers.provider.getBlock("latest");
    const actualBaseFeeWei = block?.baseFeePerGas || 0n;
    const actualBaseFeeGwei = Number(ethers.formatUnits(actualBaseFeeWei, "gwei"));

    // Compute expected dynamic challenge window (mirrors contract logic)
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

  // ── Write CSV ──
  fs.writeFileSync(csvPath, rows.join("\n") + "\n");
  console.log(`\n  ✅ Output: ${csvPath}`);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
