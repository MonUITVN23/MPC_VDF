use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use ethers::abi::{encode, Token};
use ethers::prelude::*;
use ethers::types::transaction::eip2718::TypedTransaction;
use tokio::time::timeout;
use tracing::{error, info, warn};

abigen!(
    RandomSender,
    r#"[
        function relayVDFPayload(uint256 requestId, bytes y, bytes pi, bytes seedCollective, bytes modulus, bytes blsSignature, uint8 bridgeId) payable
    ]"#
);

abigen!(
    IAxelarGasService,
    r#"[
        function estimateGasFee(string destinationChain, string destinationAddress, bytes payload, uint256 executionGasLimit, bytes params) view returns (uint256)
    ]"#
);

pub type WalletSigner = SignerMiddleware<Provider<Http>, Wallet<k256::ecdsa::SigningKey>>;

#[derive(Debug, Clone)]
pub struct RelayPayload {
    pub request_id: u64,
    pub bridge_id: u8,
    pub y: Vec<u8>,
    pub pi: Vec<u8>,
    pub seed_collective: Vec<u8>,
    pub modulus: Vec<u8>,
    pub aggregate_signature: Vec<u8>,
    pub cross_chain_fee_wei: U256,
}

#[async_trait]
pub trait BridgeRelayer {
    async fn relay_payload(&self, payload: RelayPayload) -> Result<H256>;
}

pub struct AxelarRelayer {
    signer: Arc<WalletSigner>,
    sender_address: Address,
    gas_service_address: Address,
    destination_chain: String,
    destination_address: String,
    execution_gas_limit: U256,
    estimate_params: Bytes,
    fee_buffer_bps: u32,
    max_fee_cap_wei: U256,
    daily_budget_wei: U256,
    budget_state: Mutex<BudgetState>,
}

#[derive(Debug)]
struct BudgetState {
    day_index: u64,
    spent_today_wei: U256,
}

fn current_day_index_utc() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs() / 86_400)
        .unwrap_or_default()
}

impl AxelarRelayer {
    pub fn new(
        signer: Arc<WalletSigner>,
        sender_address: Address,
        gas_service_address: Address,
        destination_chain: String,
        destination_address: String,
        execution_gas_limit: U256,
        estimate_params: Bytes,
        fee_buffer_bps: u32,
        max_fee_cap_wei: U256,
        daily_budget_wei: U256,
    ) -> Self {
        let day_index = current_day_index_utc();
        Self {
            signer,
            sender_address,
            gas_service_address,
            destination_chain,
            destination_address,
            execution_gas_limit,
            estimate_params,
            fee_buffer_bps,
            max_fee_cap_wei,
            daily_budget_wei,
            budget_state: Mutex::new(BudgetState {
                day_index,
                spent_today_wei: U256::zero(),
            }),
        }
    }
}

#[async_trait]
impl BridgeRelayer for AxelarRelayer {
    async fn relay_payload(&self, payload: RelayPayload) -> Result<H256> {
        let sender = RandomSender::new(self.sender_address, self.signer.clone());

        let relay_payload_abi = encode(&[
            Token::Uint(payload.request_id.into()),
            Token::Bytes(payload.y.clone()),
            Token::Bytes(payload.pi.clone()),
            Token::Bytes(payload.seed_collective.clone()),
            Token::Bytes(payload.modulus.clone()),
            Token::Bytes(payload.aggregate_signature.clone()),
        ]);

        let gas_service = IAxelarGasService::new(self.gas_service_address, self.signer.clone());
        let estimated_fee_result = gas_service
            .estimate_gas_fee(
                self.destination_chain.clone(),
                self.destination_address.clone(),
                relay_payload_abi.into(),
                self.execution_gas_limit,
                self.estimate_params.clone(),
            )
            .call()
            .await;

        let (fee_to_pay, fallback_used) = match estimated_fee_result {
            Ok(estimated_fee) => {
                let buffered_fee = estimated_fee
                    .saturating_mul(U256::from(self.fee_buffer_bps))
                    / U256::from(10_000u64);
                (buffered_fee, false)
            }
            Err(error) => {
                warn!(
                    error = %error,
                    fallback_fee_wei = %payload.cross_chain_fee_wei,
                    "Axelar estimateGasFee failed, using fallback fee"
                );
                (payload.cross_chain_fee_wei, true)
            }
        };

        info!(
            "Axelar Dynamic Fee estimated: {} wei (Fallback used: {})",
            fee_to_pay,
            fallback_used
        );

        if fee_to_pay > self.max_fee_cap_wei {
            error!(
                estimated_fee_wei = %fee_to_pay,
                fee_cap_wei = %self.max_fee_cap_wei,
                "Fee exceeds cap, skipping request to prevent wallet drain"
            );
            bail!(
                "fee exceeds cap: estimated={} cap={}",
                fee_to_pay,
                self.max_fee_cap_wei
            );
        }

        if self.daily_budget_wei > U256::zero() {
            let mut state = self
                .budget_state
                .lock()
                .map_err(|_| anyhow::anyhow!("budget watchdog mutex poisoned"))?;

            let today = current_day_index_utc();
            if state.day_index != today {
                state.day_index = today;
                state.spent_today_wei = U256::zero();
            }

            let projected_spent = state.spent_today_wei.saturating_add(fee_to_pay);
            if projected_spent > self.daily_budget_wei {
                error!(
                    fee_to_pay_wei = %fee_to_pay,
                    spent_today_wei = %state.spent_today_wei,
                    projected_spent_wei = %projected_spent,
                    daily_budget_wei = %self.daily_budget_wei,
                    "Daily budget exceeded, skipping request to prevent wallet drain"
                );
                bail!(
                    "daily budget exceeded: projected={} budget={}",
                    projected_spent,
                    self.daily_budget_wei
                );
            }

            state.spent_today_wei = projected_spent;

            info!(
                spent_today_wei = %state.spent_today_wei,
                daily_budget_wei = %self.daily_budget_wei,
                remaining_budget_wei = %self.daily_budget_wei.saturating_sub(state.spent_today_wei),
                "Budget watchdog reservation recorded"
            );
        }

        let call = sender
            .relay_vdf_payload(
                payload.request_id.into(),
                payload.y.into(),
                payload.pi.into(),
                payload.seed_collective.into(),
                payload.modulus.into(),
                payload.aggregate_signature.into(),
                1u8,
            )
            .value(fee_to_pay);

        let calldata = call
            .calldata()
            .ok_or_else(|| anyhow::anyhow!("failed encoding relayVDFPayload calldata"))?;

        let tx: TypedTransaction = Eip1559TransactionRequest {
            to: Some(NameOrAddress::Address(self.sender_address)),
            data: Some(calldata),
            value: Some(fee_to_pay),
            gas: Some(U256::from(900_000u64)),
            max_priority_fee_per_gas: Some(U256::from(40_000_000_000u64)),
            max_fee_per_gas: Some(U256::from(60_000_000_000u64)),
            ..Default::default()
        }
        .into();

        let pending = self
            .signer
            .send_transaction(tx, None)
            .await
            .context("failed to send relayVDFPayload tx via Axelar")?;
        let tx_hash = pending.tx_hash();

        let receipt = pending
            .await
            .context("failed waiting relayVDFPayload receipt via Axelar")?
            .ok_or_else(|| anyhow::anyhow!("relayVDFPayload dropped from mempool via Axelar"))?;

        if receipt.status != Some(U64::from(1u64)) {
            bail!("relayVDFPayload reverted via Axelar: tx={tx_hash:?}");
        }

        Ok(tx_hash)
    }
}

pub struct LayerZeroMockRelayer;

impl LayerZeroMockRelayer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl BridgeRelayer for LayerZeroMockRelayer {
    async fn relay_payload(&self, payload: RelayPayload) -> Result<H256> {
        info!(
            request_id = payload.request_id,
            bridge_id = 2u8,
            fee_wei = %payload.cross_chain_fee_wei,
            "Pretending to route via LayerZero..."
        );

        let fake = H256::from_low_u64_be((payload.request_id << 8) | 2u64);
        Ok(fake)
    }
}

pub struct MultiBridgeRouter {
    relayers: Vec<(u8, Box<dyn BridgeRelayer + Send + Sync>)>,
    per_bridge_timeout: Duration,
}

impl MultiBridgeRouter {
    pub fn new(relayers: Vec<(u8, Box<dyn BridgeRelayer + Send + Sync>)>) -> Self {
        Self {
            relayers,
            per_bridge_timeout: Duration::from_secs(15),
        }
    }

    pub fn with_timeout(
        relayers: Vec<(u8, Box<dyn BridgeRelayer + Send + Sync>)>,
        per_bridge_timeout: Duration,
    ) -> Self {
        Self {
            relayers,
            per_bridge_timeout,
        }
    }

    pub fn default_with_priority(
        axelar: AxelarRelayer,
        layerzero_mock: LayerZeroMockRelayer,
    ) -> Self {
        Self::new(vec![
            (1u8, Box::new(axelar)),
            (2u8, Box::new(layerzero_mock)),
        ])
    }

    pub async fn execute_with_failover(&self, payload: RelayPayload) -> Result<(H256, u8)> {
        let mut errors = Vec::new();

        for (bridge_id, relayer) in &self.relayers {
            let t4_start = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|v| v.as_millis() as u64)
                .unwrap_or_default();

            info!(
                t4_start = t4_start,
                bridge_id = *bridge_id,
                fee_wei = %payload.cross_chain_fee_wei,
                "starting bridge relay attempt"
            );

            let mut attempt_payload = payload.clone();
            attempt_payload.bridge_id = *bridge_id;

            match timeout(self.per_bridge_timeout, relayer.relay_payload(attempt_payload)).await {
                Ok(Ok(tx_hash)) => {
                    info!(bridge_id = *bridge_id, tx_hash = ?tx_hash, "bridge relay success");
                    return Ok((tx_hash, *bridge_id));
                }
                Ok(Err(err)) => {
                    warn!(bridge_id = *bridge_id, error = %err, "bridge relay failed, trying next bridge");
                    errors.push(format!("bridge_id={bridge_id}: {err}"));
                }
                Err(_) => {
                    warn!(bridge_id = *bridge_id, "bridge relay timeout, trying next bridge");
                    errors.push(format!("bridge_id={bridge_id}: timeout"));
                }
            }
        }

        bail!("all bridges failed: {}", errors.join(" | "));
    }
}
