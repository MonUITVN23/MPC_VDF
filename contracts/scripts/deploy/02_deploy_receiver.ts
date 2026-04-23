import { ethers, network } from "hardhat";

const BN254_G2_X: [bigint, bigint] = [
  11559732032986387107991004021392285783925812861821192530917403151452391805634n,
  10857046999023057135944570762232829481370756359578518086990519993285655852781n,
];

const BN254_G2_Y: [bigint, bigint] = [
  4082367875863433681332203403145435568316851327593401208105741076214120093531n,
  8495653923123431417604973247489272438418190587263600148770280649306958101930n,
];

function resolveGatewayAddress(): string {
  if (network.name === "amoy") {
    const gateway = process.env.AXELAR_GATEWAY_AMOY;
    if (!gateway) {
      throw new Error("Missing AXELAR_GATEWAY_AMOY in ../.env");
    }
    return gateway;
  }

  if (network.name === "sepolia") {
    const gateway = process.env.AXELAR_GATEWAY_SEPOLIA;
    if (!gateway) {
      throw new Error("Missing AXELAR_GATEWAY_SEPOLIA in ../.env");
    }
    return gateway;
  }

  throw new Error(`Unsupported network for receiver deploy: ${network.name}`);
}

async function main() {
  const gateway = resolveGatewayAddress();
  console.log(`Deploying RandomReceiver to network: ${network.name}`);
  console.log(`Axelar gateway: ${gateway}`);

  const Factory = await ethers.getContractFactory("RandomReceiver");
  const contract = await Factory.deploy(gateway);
  await contract.waitForDeployment();

  const address = await contract.getAddress();
  const deployTx = contract.deploymentTransaction();

  console.log("Deploying Halo2Verifier...");
  const VerifierFactory = await ethers.getContractFactory("Halo2Verifier");
  const verifier = await VerifierFactory.deploy();
  await verifier.waitForDeployment();
  const verifierAddress = await verifier.getAddress();
  console.log(`Halo2Verifier deployed to: ${verifierAddress}`);

  console.log("Configuring RandomReceiver...");
  const setVerifierTx = await (contract as any).setZkVerifier(verifierAddress);
  await setVerifierTx.wait();
  
  const setModeTx = await (contract as any).setZkProofMode(true);
  await setModeTx.wait();

  
  const setPkTx = await (contract as any).setAggregatePublicKey(BN254_G2_X, BN254_G2_Y);
  await setPkTx.wait();

  console.log("RandomReceiver deployed successfully");
  console.log(`Address: ${address}`);
  console.log(`Tx Hash: ${deployTx?.hash ?? "N/A"}`);
  console.log(`setAggregatePublicKey Tx Hash: ${setPkTx.hash}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
