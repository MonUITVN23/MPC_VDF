use std::sync::Arc;

use anyhow::{bail, Context, Result};
use ethers::prelude::*;

abigen!(
	RandomReceiver,
	r#"[
		function submitOptimisticResult(uint256 requestId, bytes y, bytes pi, bytes seedCollective, bytes aggregateSignature)
	]"#
);

pub type WalletSigner = SignerMiddleware<Provider<Http>, Wallet<k256::ecdsa::SigningKey>>;

#[derive(Debug, Clone)]
pub struct RelayPayload {
	pub request_id: u64,
	pub y: Vec<u8>,
	pub pi: Vec<u8>,
	pub seed_collective: Vec<u8>,
	pub aggregate_signature: Vec<u8>,
}

pub async fn relay_payload_to_receiver(
	signer: Arc<WalletSigner>,
	receiver_address: Address,
	payload: RelayPayload,
) -> Result<H256> {
	let receiver = RandomReceiver::new(receiver_address, signer.clone());

	let call = receiver
		.submit_optimistic_result(
		payload.request_id.into(),
		payload.y.into(),
		payload.pi.into(),
		payload.seed_collective.into(),
		payload.aggregate_signature.into(),
	)
		.gas(U256::from(800_000u64));

	let pending = call
		.send()
		.await
		.context("failed to send submitOptimisticResult tx")?;
	let tx_hash = pending.tx_hash();

	let receipt = pending
		.await
		.context("failed waiting submitOptimisticResult receipt")?
		.ok_or_else(|| anyhow::anyhow!("submitOptimisticResult dropped from mempool"))?;

	if receipt.status != Some(U64::from(1u64)) {
		bail!("submitOptimisticResult reverted on-chain: tx={tx_hash:?}");
	}

	Ok(tx_hash)
}
