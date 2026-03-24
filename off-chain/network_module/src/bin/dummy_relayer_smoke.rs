use anyhow::Result;
use ethers::types::U256;

use network_module::bridges::{BridgeRelayer, RelayPayload};
use network_module::relayers::dummy::DummyBridgeRelayer;

#[tokio::main]
async fn main() -> Result<()> {
    let relayer = DummyBridgeRelayer::new("DUMMY");

    let payload = RelayPayload {
        request_id: 20260324,
        bridge_id: 0,
        y: vec![0x01, 0x02],
        pi: vec![0x03],
        seed_collective: vec![0x04],
        modulus: vec![0x05],
        aggregate_signature: vec![0x06],
        cross_chain_fee_wei: U256::from(123_456u64),
    };

    let tx_hash = relayer.relay_payload(payload).await?;
    println!("Dummy plugin '{}' produced tx_hash={:#x}", relayer.plugin_name(), tx_hash);

    Ok(())
}
