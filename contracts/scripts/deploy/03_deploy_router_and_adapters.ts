import * as fs from "fs";
import * as path from "path";
import { ethers, network } from "hardhat";
import * as dotenv from "dotenv";

dotenv.config({ path: "../.env" });

type BridgeKind = "axelar" | "layerzero" | "wormhole";

type EnvRef = {
  env: string;
  default?: string;
  required?: boolean;
};

type ProtocolMappings = {
  layerZeroEidByEvmChainId: Record<string, number>;
  wormholeChainByEvmChainId: Record<string, number>;
};

type BridgeDefinition = {
  name: string;
  kind: BridgeKind;
  enabled?: boolean;

  axelarGateway?: EnvRef;
  axelarGasService?: EnvRef;
  axelarFeeHintWei?: EnvRef;

  layerZeroEndpoint?: EnvRef;
  layerZeroDstEid?: EnvRef;
  layerZeroOptionsHex?: EnvRef;

  wormholeRelayer?: EnvRef;
  wormholeTargetChain?: EnvRef;
  wormholeGasLimit?: EnvRef;
  wormholeRefundChain?: EnvRef;
  wormholeRefundAddress?: EnvRef;
  wormholeFeeHintWei?: EnvRef;
  wormholeFeeOracle?: EnvRef;
};

type BridgeDeployConfig = {
  networkName: string;
  destinationChain: string;
  destinationAddressEnv: string;
  destinationEvmChainIdEnv: string;
  destinationEvmChainIdDefault: string;
  protocolMappings: ProtocolMappings;
  bridges: BridgeDefinition[];
};

type DeployContext = {
  destinationChain: string;
  destinationAddress: string;
  sourceEvmChainId: number;
  destinationEvmChainId: number;
  routerAddress: string;
  deployerAddress: string;
  protocolMappings: ProtocolMappings;
};

type DeployedAdapter = {
  name: string;
  kind: BridgeKind;
  bridgeId: string;
  address: string;
  txHash?: string;
};

const DEFAULT_CONFIG_PATH = path.resolve(__dirname, "../../config/bridges.sepolia.json");

function envOrDefault(name: string, fallback: string): string {
  const value = process.env[name];
  return value && value.trim().length > 0 ? value : fallback;
}

function parseU32(name: string, value: string): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 0 || parsed > 0xffffffff) {
    throw new Error(`Invalid uint32 for ${name}: ${value}`);
  }
  return parsed;
}

function parseU16(name: string, value: string): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 0 || parsed > 0xffff) {
    throw new Error(`Invalid uint16 for ${name}: ${value}`);
  }
  return parsed;
}

function parseU256(name: string, value: string): bigint {
  try {
    return BigInt(value);
  } catch {
    throw new Error(`Invalid uint256 for ${name}: ${value}`);
  }
}

function resolveEnv(ref: EnvRef | undefined, label: string): string {
  if (!ref) {
    throw new Error(`Missing env reference config for ${label}`);
  }

  const value = process.env[ref.env];
  if (value && value.trim().length > 0) {
    return value;
  }

  if (ref.default !== undefined) {
    return ref.default;
  }

  if (ref.required !== false) {
    throw new Error(`Missing env var ${ref.env} for ${label}`);
  }

  return "";
}

function deriveMappedChainId(
  envRef: EnvRef | undefined,
  mapName: string,
  mapping: Record<string, number>,
  evmChainId: number,
  parser: (name: string, value: string) => number
): number {
  if (envRef) {
    const resolved = resolveEnv({ ...envRef, required: false }, mapName);
    if (resolved.trim().length > 0) {
      return parser(envRef.env, resolved);
    }
  }

  const mapped = mapping[String(evmChainId)];
  if (typeof mapped !== "number") {
    throw new Error(
      `No mapping for ${mapName} with EVM chain id ${evmChainId}. Add env override or extend config mapping.`
    );
  }
  return mapped;
}

function loadConfig(): BridgeDeployConfig {
  const configPath = process.env.BRIDGE_CONFIG_PATH
    ? path.resolve(process.cwd(), process.env.BRIDGE_CONFIG_PATH)
    : DEFAULT_CONFIG_PATH;

  if (!fs.existsSync(configPath)) {
    throw new Error(`Bridge config file not found: ${configPath}`);
  }

  const raw = fs.readFileSync(configPath, "utf8");
  const config = JSON.parse(raw) as BridgeDeployConfig;

  if (!config.networkName || !config.destinationAddressEnv || !config.bridges?.length) {
    throw new Error(`Invalid bridge config schema in ${configPath}`);
  }

  console.log(`Using bridge config: ${configPath}`);
  return config;
}

async function deployAxelarAdapter(def: BridgeDefinition, ctx: DeployContext): Promise<DeployedAdapter> {
  const gateway = resolveEnv(def.axelarGateway, `${def.name}.axelarGateway`);
  const gasService = resolveEnv(def.axelarGasService, `${def.name}.axelarGasService`);
  const feeHintWei = resolveEnv(def.axelarFeeHintWei, `${def.name}.axelarFeeHintWei`);

  const Factory = await ethers.getContractFactory("AxelarAdapter");
  const adapter = await Factory.deploy(gateway, gasService, ctx.routerAddress, feeHintWei);
  await adapter.waitForDeployment();

  return {
    name: def.name,
    kind: def.kind,
    bridgeId: ethers.id(def.name),
    address: await adapter.getAddress(),
  };
}

async function deployLayerZeroAdapter(def: BridgeDefinition, ctx: DeployContext): Promise<DeployedAdapter> {
  const endpoint = resolveEnv(def.layerZeroEndpoint, `${def.name}.layerZeroEndpoint`);
  const dstEid = deriveMappedChainId(
    def.layerZeroDstEid,
    `${def.name}.layerZeroDstEid`,
    ctx.protocolMappings.layerZeroEidByEvmChainId,
    ctx.destinationEvmChainId,
    parseU32
  );
  const optionsHex = resolveEnv(
    def.layerZeroOptionsHex ?? { env: "LZ_OPTIONS_HEX", default: "0x", required: false },
    `${def.name}.layerZeroOptionsHex`
  );

  const Factory = await ethers.getContractFactory("LayerZeroAdapter");
  const adapter = await Factory.deploy(endpoint, ctx.routerAddress);
  await adapter.waitForDeployment();
  const address = await adapter.getAddress();

  const receiverBytes32 = ethers.zeroPadValue(ctx.destinationAddress, 32);
  const routeTx = await (adapter as any).setRoute(
    ctx.destinationChain,
    ctx.destinationAddress,
    dstEid,
    receiverBytes32,
    ethers.getBytes(optionsHex)
  );
  await routeTx.wait();

  return {
    name: def.name,
    kind: def.kind,
    bridgeId: ethers.id(def.name),
    address,
    txHash: routeTx.hash,
  };
}

async function deployWormholeAdapter(def: BridgeDefinition, ctx: DeployContext): Promise<DeployedAdapter> {
  const relayer = resolveEnv(def.wormholeRelayer, `${def.name}.wormholeRelayer`);
  const targetChain = deriveMappedChainId(
    def.wormholeTargetChain,
    `${def.name}.wormholeTargetChain`,
    ctx.protocolMappings.wormholeChainByEvmChainId,
    ctx.destinationEvmChainId,
    parseU16
  );
  const refundChain = deriveMappedChainId(
    def.wormholeRefundChain,
    `${def.name}.wormholeRefundChain`,
    ctx.protocolMappings.wormholeChainByEvmChainId,
    ctx.sourceEvmChainId,
    parseU16
  );
  const gasLimit = parseU256(
    def.wormholeGasLimit?.env ?? `${def.name}.wormholeGasLimit`,
    resolveEnv(def.wormholeGasLimit, `${def.name}.wormholeGasLimit`)
  );
  const refundAddress = resolveEnv(
    def.wormholeRefundAddress ?? { env: "WORMHOLE_REFUND_ADDRESS", default: ctx.deployerAddress, required: false },
    `${def.name}.wormholeRefundAddress`
  ) || ctx.deployerAddress;
  const feeHintWei = parseU256(
    def.wormholeFeeHintWei?.env ?? `${def.name}.wormholeFeeHintWei`,
    resolveEnv(
      def.wormholeFeeHintWei ?? { env: "WORMHOLE_FEE_HINT_WEI", default: "200000000000000", required: false },
      `${def.name}.wormholeFeeHintWei`
    )
  );
  const feeOracle = resolveEnv(
    def.wormholeFeeOracle ?? { env: "WORMHOLE_FEE_ORACLE", default: relayer, required: false },
    `${def.name}.wormholeFeeOracle`
  ) || relayer;

  const Factory = await ethers.getContractFactory("WormholeAdapter");
  const adapter = await Factory.deploy(relayer, ctx.routerAddress);
  await adapter.waitForDeployment();
  const address = await adapter.getAddress();

  const routeTx = await (adapter as any).setRoute(
    ctx.destinationChain,
    ctx.destinationAddress,
    targetChain,
    ctx.destinationAddress,
    gasLimit,
    refundChain,
    refundAddress
  );
  await routeTx.wait();

  const feeHintTx = await (adapter as any).setFeeHint(feeHintWei);
  await feeHintTx.wait();

  const feeOracleTx = await (adapter as any).setFeeOracle(feeOracle);
  await feeOracleTx.wait();

  return {
    name: def.name,
    kind: def.kind,
    bridgeId: ethers.id(def.name),
    address,
    txHash: routeTx.hash,
  };
}

async function main() {
  const config = loadConfig();
  if (network.name !== config.networkName) {
    throw new Error(`Config is for ${config.networkName} but active network is ${network.name}`);
  }

  const destinationChain = config.destinationChain;
  const destinationAddress = process.env[config.destinationAddressEnv];
  if (!destinationAddress || destinationAddress.trim().length === 0) {
    throw new Error(`Missing destination address env: ${config.destinationAddressEnv}`);
  }

  const destinationEvmChainId = parseU32(
    config.destinationEvmChainIdEnv,
    envOrDefault(config.destinationEvmChainIdEnv, config.destinationEvmChainIdDefault)
  );
  const [deployer] = await ethers.getSigners();
  const sourceEvmChainId = Number((await ethers.provider.getNetwork()).chainId);

  const RouterFactory = await ethers.getContractFactory("RandomRouter");
  const router = await RouterFactory.deploy(destinationChain, destinationAddress);
  await router.waitForDeployment();
  const routerAddress = await router.getAddress();

  const ctx: DeployContext = {
    destinationChain,
    destinationAddress,
    sourceEvmChainId,
    destinationEvmChainId,
    routerAddress,
    deployerAddress: deployer.address,
    protocolMappings: config.protocolMappings,
  };

  console.log("Deploying router and adapters from config...");
  console.log(`Source EVM chainId: ${sourceEvmChainId}`);
  console.log(`Dest EVM chainId:   ${destinationEvmChainId}`);
  console.log(`Destination chain:  ${destinationChain}`);
  console.log(`Destination addr:   ${destinationAddress}`);

  const deployed: DeployedAdapter[] = [];
  const registerTxs: Array<{ name: string; txHash: string }> = [];

  for (const bridge of config.bridges) {
    if (bridge.enabled === false) {
      console.log(`Skip disabled bridge: ${bridge.name}`);
      continue;
    }

    let item: DeployedAdapter;
    if (bridge.kind === "axelar") {
      item = await deployAxelarAdapter(bridge, ctx);
    } else if (bridge.kind === "layerzero") {
      item = await deployLayerZeroAdapter(bridge, ctx);
    } else if (bridge.kind === "wormhole") {
      item = await deployWormholeAdapter(bridge, ctx);
    } else {
      throw new Error(`Unsupported bridge kind: ${bridge.kind}`);
    }

    const tx = await router.registerAdapter(item.bridgeId, item.address);
    await tx.wait();
    registerTxs.push({ name: bridge.name, txHash: tx.hash });
    deployed.push(item);
  }

  console.log("Deployment completed.");
  console.log("----------------------------------------");
  console.log(`RandomRouter: ${routerAddress}`);
  for (const adapter of deployed) {
    console.log(`${adapter.name} (${adapter.kind})`);
    console.log(`  bridgeId: ${adapter.bridgeId}`);
    console.log(`  adapter:  ${adapter.address}`);
    if (adapter.txHash) {
      console.log(`  routeTx:  ${adapter.txHash}`);
    }
  }
  console.log("----------------------------------------");
  for (const tx of registerTxs) {
    console.log(`register ${tx.name}: ${tx.txHash}`);
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
