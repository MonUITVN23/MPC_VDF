# Bridge Deploy Config

`03_deploy_router_and_adapters.ts` now reads bridge definitions from JSON config instead of hardcoded values.

## Default config

- File: `contracts/config/bridges.sepolia.json`
- Override path with env:

```bash
BRIDGE_CONFIG_PATH=contracts/config/bridges.sepolia.json
```

## Run deploy

```bash
cd /home/xuananh/mpc-vdf/contracts
npx hardhat run scripts/deploy/03_deploy_router_and_adapters.ts --network sepolia
```

## Philosophy

- Business chains stay fixed by project context (`source EVM chain` and `destination EVM chain`).
- Bridge transports (Axelar/LayerZero/Wormhole/others) are plugin entries in JSON config.
- Add bridge by adding new entry in `bridges` list and required env values.

## Required config fields

Top-level:

- `networkName`
- `destinationChain`
- `destinationAddressEnv`
- `destinationEvmChainIdEnv`
- `destinationEvmChainIdDefault`
- `protocolMappings`
- `bridges`

Bridge entry:

- `name` (used to compute `bridgeId = ethers.id(name)`)
- `kind` (`axelar`, `layerzero`, `wormhole`)
- `enabled` (optional)
- kind-specific env refs

Env ref object format:

```json
{ "env": "ENV_NAME", "default": "optional-default", "required": true }
```

If an env ref has no value and no default, deploy script fails fast.
