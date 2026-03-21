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

function formatUnixTime(seconds: bigint): string {
  if (seconds === 0n) return "0";
  return `${seconds.toString()} (${new Date(Number(seconds) * 1000).toISOString()})`;
}

async function main() {
  const receiverAddress = requiredEnv("RANDOM_RECEIVER_ADDRESS");
  const requestId = process.env.REQUEST_ID ?? process.env.CHECK_REQUEST_ID;

  if (!requestId) {
    throw new Error("Missing env var: REQUEST_ID (or CHECK_REQUEST_ID)");
  }

  console.log(`Network: ${network.name}`);
  if (network.name !== "amoy") {
    throw new Error(`This script must run on amoy. Current network: ${network.name}`);
  }

  const receiver = await ethers.getContractAt("RandomReceiver", receiverAddress);
  const item = await (receiver as any).queue(BigInt(requestId));

  if (item.submittedAt === 0n) {
    console.log(`requestId: ${requestId}`);
    console.log("queue(requestId): empty");
    return;
  }

  console.log(`RandomReceiver: ${receiverAddress}`);
  console.log(`requestId: ${requestId}`);
  console.log(`y: ${item.y}`);
  console.log(`pi: ${item.pi}`);
  console.log(`submittedAt: ${formatUnixTime(item.submittedAt)}`);
  console.log(`challengeDeadline: ${formatUnixTime(item.challengeDeadline)}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
