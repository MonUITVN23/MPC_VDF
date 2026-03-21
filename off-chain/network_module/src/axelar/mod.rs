use std::sync::Arc;

use anyhow::{bail, Context, Result};
use ethers::prelude::*;
use ethers::types::transaction::eip2718::TypedTransaction;

abigen!(
	RandomSender,
	r#"[
		function relayVDFPayload(uint256 requestId, bytes y, bytes pi, bytes seedCollective, bytes modulus, bytes blsSignature) payable
	]"#
);

pub type WalletSigner = SignerMiddleware<Provider<Http>, Wallet<k256::ecdsa::SigningKey>>;

#[derive(Debug, Clone)]
pub struct RelayPayload {
	pub request_id: u64,
	pub y: Vec<u8>,
	pub pi: Vec<u8>,
	pub seed_collective: Vec<u8>,
	pub modulus: Vec<u8>,
	pub aggregate_signature: Vec<u8>,
	pub axelar_native_gas_fee_wei: U256,
}

pub async fn relay_payload_to_sender(
	signer: Arc<WalletSigner>,
	sender_address: Address,
	payload: RelayPayload,
) -> Result<H256> {
	let sender = RandomSender::new(sender_address, signer.clone());

	let call = sender
		.relay_vdf_payload(
		payload.request_id.into(),
		payload.y.into(),
		payload.pi.into(),
		payload.seed_collective.into(),
		payload.modulus.into(),
		payload.aggregate_signature.into(),
	)
		.value(payload.axelar_native_gas_fee_wei);

	let calldata = call
		.calldata()
		.ok_or_else(|| anyhow::anyhow!("failed encoding relayVDFPayload calldata"))?;

	let tx: TypedTransaction = Eip1559TransactionRequest {
		to: Some(NameOrAddress::Address(sender_address)),
		data: Some(calldata),
		value: Some(payload.axelar_native_gas_fee_wei),
		gas: Some(U256::from(800_000u64)),
		max_priority_fee_per_gas: Some(U256::from(40_000_000_000u64)),
		max_fee_per_gas: Some(U256::from(60_000_000_000u64)),
		..Default::default()
	}
	.into();

	let pending = signer
		.send_transaction(tx, None)
		.await
		.context("failed to send relayVDFPayload tx")?;
	let tx_hash = pending.tx_hash();

	let receipt = pending
		.await
		.context("failed waiting relayVDFPayload receipt")?
		.ok_or_else(|| anyhow::anyhow!("relayVDFPayload dropped from mempool"))?;

	if receipt.status != Some(U64::from(1u64)) {
		bail!("relayVDFPayload reverted on-chain: tx={tx_hash:?}");
	}

	Ok(tx_hash)
}
