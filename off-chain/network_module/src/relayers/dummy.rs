use anyhow::Result;
use async_trait::async_trait;
use ethers::types::H256;
use ethers::utils::keccak256;

use crate::bridges::{BridgeRelayer, RelayPayload};

pub struct DummyBridgeRelayer {
    plugin_name: String,
}

impl DummyBridgeRelayer {
    pub fn new(plugin_name: impl Into<String>) -> Self {
        Self {
            plugin_name: plugin_name.into(),
        }
    }

    pub fn plugin_name(&self) -> &str {
        &self.plugin_name
    }
}

#[async_trait]
impl BridgeRelayer for DummyBridgeRelayer {
    async fn relay_payload(&self, payload: RelayPayload) -> Result<H256> {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(self.plugin_name.as_bytes());
        encoded.extend_from_slice(&payload.request_id.to_be_bytes());
        encoded.extend_from_slice(payload.cross_chain_fee_wei.as_u128().to_be_bytes().as_slice());
        let digest = keccak256(encoded);
        Ok(H256::from(digest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::types::U256;

    #[tokio::test]
    async fn dummy_returns_stable_hash() {
        let relayer = DummyBridgeRelayer::new("DUMMY");
        let payload = RelayPayload {
            request_id: 42,
            bridge_id: 0,
            y: vec![1, 2, 3],
            pi: vec![4, 5],
            seed_collective: vec![6],
            modulus: vec![7],
            aggregate_signature: vec![8],
            cross_chain_fee_wei: U256::from(123u64),
            zk_proof_data: Vec::new(),
            zk_public_signals: [U256::zero(); 7],
        };

        let first = relayer
            .relay_payload(payload.clone())
            .await
            .expect("first relay hash should be produced");
        let second = relayer
            .relay_payload(payload)
            .await
            .expect("second relay hash should be produced");

        assert_eq!(first, second);
    }
}
