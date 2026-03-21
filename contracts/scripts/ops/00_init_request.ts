import { network, ethers } from "hardhat";
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
  const senderAddress = requiredEnv("RANDOM_SENDER_ADDRESS");
  const userSeed = process.env.USER_SEED ?? Math.floor(Date.now() / 1000).toString();

  console.log(`Network: ${network.name}`);
  if (network.name !== "sepolia") {
    throw new Error(`This script must run on sepolia. Current network: ${network.name}`);
  }

  const sender = await ethers.getContractAt("RandomSender", senderAddress);
  const tx = await (sender as any).requestRandomness(BigInt(userSeed));
  const receipt = await tx.wait();

  const log = receipt?.logs.find((l: any) => l.topics[0] === sender.interface.getEvent("LogRequest")?.topicHash);
  if (!log) {
    console.log(`Tx hash: ${tx.hash}`);
    console.log("No LogRequest event decoded from receipt logs.");
    return;
  }

  const parsed = sender.interface.parseLog({
    topics: log.topics,
    data: log.data,
  });

  console.log(`Tx hash: ${tx.hash}`);
  console.log(`requestId: ${parsed?.args.requestId.toString()}`);
  console.log(`userSeed: ${parsed?.args.userSeed.toString()}`);
  console.log(`export REQUEST_ID=${parsed?.args.requestId.toString()}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
