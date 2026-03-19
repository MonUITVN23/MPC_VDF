import { ethers, network } from "hardhat";
import * as dotenv from "dotenv";

dotenv.config({ path: "../.env" });

const AXELAR_GAS_SERVICE_SEPOLIA_DEFAULT = "0xbE406F0189A0B4cf3A05C286473D23791Dd44Cc6";

function requiredEnv(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`Missing env var: ${name}`);
  }
  return value;
}

async function main() {
  if (network.name !== "sepolia") {
    throw new Error(`RandomSender deploy script must run on sepolia. Current network: ${network.name}`);
  }

  const gateway = requiredEnv("AXELAR_GATEWAY_SEPOLIA");
  const gasService = process.env.AXELAR_GAS_SERVICE_SEPOLIA ?? AXELAR_GAS_SERVICE_SEPOLIA_DEFAULT;
  const destinationAddress = requiredEnv("RANDOM_RECEIVER_ADDRESS");

  console.log(`Deploying RandomSender to network: ${network.name}`);
  console.log(`Axelar gateway: ${gateway}`);
  console.log(`Axelar gas service: ${gasService}`);
  console.log(`Destination receiver address: ${destinationAddress}`);

  const Factory = await ethers.getContractFactory("RandomSender");
  const contract = await Factory.deploy(gateway, gasService, destinationAddress);
  await contract.waitForDeployment();

  const address = await contract.getAddress();
  const deployTx = contract.deploymentTransaction();

  console.log("RandomSender deployed successfully");
  console.log(`Address: ${address}`);
  console.log(`Tx Hash: ${deployTx?.hash ?? "N/A"}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
