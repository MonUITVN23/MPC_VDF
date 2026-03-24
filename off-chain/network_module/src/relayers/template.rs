use anyhow::{bail, Result};
use async_trait::async_trait;
use ethers::types::{Address, H256, U256};

use crate::bridges::{BridgeRelayer, RelayPayload, WalletSigner};

pub struct TemplateRelayerConfig {
    pub plugin_name: String,
    pub router_address: Address,
    pub fee_buffer_bps: u32,
    pub max_fee_cap_wei: U256,
}

pub struct TemplateRelayer {
    signer: std::sync::Arc<WalletSigner>,
    config: TemplateRelayerConfig,
}

impl TemplateRelayer {
    pub fn new(
        signer: std::sync::Arc<WalletSigner>,
        config: TemplateRelayerConfig,
    ) -> Self {
        Self { signer, config }
    }

    pub fn plugin_name(&self) -> &str {
        &self.config.plugin_name
    }
}

#[async_trait]
impl BridgeRelayer for TemplateRelayer {
    async fn relay_payload(&self, payload: RelayPayload) -> Result<H256> {
        let _ = (&self.signer, &self.config.router_address, payload);
        bail!(
            "template relayer '{}' is a scaffold only; implement relay logic before use",
            self.config.plugin_name
        )
    }
}
