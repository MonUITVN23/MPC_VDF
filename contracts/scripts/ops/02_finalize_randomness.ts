import { ethers, network } from "hardhat";
import * as dotenv from "dotenv";

dotenv.config({ path: "../.env" });

function requiredEnv(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`Missing env var: ${name}`);
  }
  return value;
}

async function main() {
  const receiverAddress = requiredEnv("RANDOM_RECEIVER_ADDRESS");
  const requestId = requiredEnv("REQUEST_ID");

  console.log(`Network: ${network.name}`);
  if (network.name !== "amoy") {
    throw new Error(`This script must run on amoy. Current network: ${network.name}`);
  }

  const receiver = await ethers.getContractAt("RandomReceiver", receiverAddress);
  const tx = await (receiver as any).finalizeRandomness(BigInt(requestId));
  const receipt = await tx.wait();

  console.log(`Tx hash: ${tx.hash}`);

  const log = receipt?.logs.find(
    (entry: any) => entry.topics[0] === receiver.interface.getEvent("RandomnessFinalized")?.topicHash
  );

  if (!log) {
    return;
  }

  const parsed = receiver.interface.parseLog({ topics: log.topics, data: log.data });
  console.log(`requestId finalized: ${parsed?.args.requestId.toString()}`);
  console.log(`finalRandomness: ${parsed?.args.finalRandomness}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
