use std::{collections::HashMap, env, sync::Arc};

use ethers::prelude::{Address, Bytes, U256};

use crate::bridges::{
    AxelarRelayer, BridgeRelayer, LayerZeroRelayer, WalletSigner, WormholeRelayer,
};
use crate::relayers::dummy::DummyBridgeRelayer;

fn env_flag_enabled(name: &str) -> bool {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .map(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

pub struct RelayerFactoryInput {
    pub signer: Arc<WalletSigner>,
    pub router_address: Address,
    pub axelar_gas_service_address: Address,
    pub axelar_destination_chain: String,
    pub axelar_destination_address: String,
    pub axelar_execution_gas_limit: U256,
    pub axelar_estimate_params: Bytes,
    pub axelar_fee_buffer_bps: u32,
    pub layerzero_fee_buffer_bps: u32,
    pub wormhole_fee_buffer_bps: u32,
    pub cross_chain_fee_cap_wei: U256,
    pub cross_chain_daily_budget_wei: U256,
}

pub fn build_builtin_relayers(
    input: RelayerFactoryInput,
) -> HashMap<String, Box<dyn BridgeRelayer + Send + Sync>> {
    let axelar_relayer = AxelarRelayer::new(
        input.signer.clone(),
        input.router_address,
        input.axelar_gas_service_address,
        input.axelar_destination_chain,
        input.axelar_destination_address,
        input.axelar_execution_gas_limit,
        input.axelar_estimate_params,
        input.axelar_fee_buffer_bps,
        input.cross_chain_fee_cap_wei,
        input.cross_chain_daily_budget_wei,
    );

    let layerzero_relayer = LayerZeroRelayer::new(
        input.signer.clone(),
        input.router_address,
        input.layerzero_fee_buffer_bps,
        input.cross_chain_fee_cap_wei,
    );

    let wormhole_relayer = WormholeRelayer::new(
        input.signer,
        input.router_address,
        input.wormhole_fee_buffer_bps,
        input.cross_chain_fee_cap_wei,
    );

    let mut available_relayers: HashMap<String, Box<dyn BridgeRelayer + Send + Sync>> = HashMap::new();
    available_relayers.insert("AXELAR".to_owned(), Box::new(axelar_relayer));
    available_relayers.insert("LAYERZERO".to_owned(), Box::new(layerzero_relayer));
    available_relayers.insert("WORMHOLE".to_owned(), Box::new(wormhole_relayer));

    if env_flag_enabled("ENABLE_DUMMY_BRIDGE") {
        available_relayers.insert(
            "DUMMY".to_owned(),
            Box::new(DummyBridgeRelayer::new("DUMMY")),
        );
    }

    available_relayers
}
