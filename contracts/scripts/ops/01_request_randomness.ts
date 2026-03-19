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
  const senderAddress = requiredEnv("RANDOM_SENDER_ADDRESS");
  const gasFeeWei = requiredEnv("AXELAR_NATIVE_GAS_FEE_WEI");
  const yHex = requiredEnv("VDF_Y_HEX");
  const piHex = requiredEnv("VDF_PI_HEX");
  const seedCollectiveHex = requiredEnv("SEED_COLLECTIVE_HEX");
  const modulusHex = requiredEnv("VDF_MODULUS_HEX");
  const blsSignatureHex = requiredEnv("BLS_SIGNATURE_HEX");

  console.log(`Network: ${network.name}`);
  if (network.name !== "sepolia") {
    throw new Error(`This script must run on sepolia. Current network: ${network.name}`);
  }

  console.log(`RandomSender: ${senderAddress}`);
  console.log(`y bytes length: ${ethers.getBytes(yHex).length}`);
  console.log(`pi bytes length: ${ethers.getBytes(piHex).length}`);
  console.log(`seedCollective bytes length: ${ethers.getBytes(seedCollectiveHex).length}`);
  console.log(`modulus bytes length: ${ethers.getBytes(modulusHex).length}`);
  console.log(`blsSignature bytes length: ${ethers.getBytes(blsSignatureHex).length}`);
  console.log(`Axelar native gas fee (wei): ${gasFeeWei}`);

  const sender = await ethers.getContractAt("RandomSender", senderAddress);
  const tx = await (sender as any).requestRandomness(
    yHex,
    piHex,
    seedCollectiveHex,
    modulusHex,
    blsSignatureHex,
    { value: BigInt(gasFeeWei) }
  );
  const receipt = await tx.wait();

  const gmpLink = `https://testnet.axelarscan.io/gmp/${tx.hash}`;
  const searchLink = `https://testnet.axelarscan.io/search?query=${tx.hash}`;

  const log = receipt?.logs.find((l: any) => l.topics[0] === sender.interface.getEvent("LogRequest")?.topicHash);
  if (!log) {
    console.log(`Tx hash: ${tx.hash}`);
    console.log(`AxelarScan GMP: ${gmpLink}`);
    console.log(`AxelarScan Search: ${searchLink}`);
    console.log("No LogRequest event decoded from receipt logs.");
    return;
  }

  const parsed = sender.interface.parseLog({
    topics: log.topics,
    data: log.data,
  });

  console.log(`Tx hash: ${tx.hash}`);
  console.log(`requestId: ${parsed?.args.requestId.toString()}`);
  console.log(`AxelarScan GMP: ${gmpLink}`);
  console.log(`AxelarScan Search: ${searchLink}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
