use std::{collections::HashSet, env, str::FromStr, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use crypto_engine::run_randomness_pipeline_with_seed;
use dotenvy::dotenv;
use ethers::prelude::*;

use network_module::axelar::{relay_payload_to_receiver, RelayPayload, WalletSigner};
use network_module::rpc::{current_block, fetch_log_requests_in_range, EthProvider};

fn env_required(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("missing required env var: {name}"))
}

fn parse_address(name: &str, value: &str) -> Result<Address> {
    Address::from_str(value).with_context(|| format!("invalid address in {name}: {value}"))
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let sepolia_rpc = env_required("SEPOLIA_RPC_URL")?;
    let destination_rpc = env::var("DEST_RPC_URL")
        .or_else(|_| env::var("AMOY_RPC_URL"))
        .with_context(|| "missing required env var: DEST_RPC_URL or AMOY_RPC_URL")?;
    let private_key = env_required("PRIVATE_KEY")?;
    let sender_addr = parse_address("RANDOM_SENDER_ADDRESS", &env_required("RANDOM_SENDER_ADDRESS")?)?;
    let receiver_addr = parse_address("RANDOM_RECEIVER_ADDRESS", &env_required("RANDOM_RECEIVER_ADDRESS")?)?;
    let vdf_t = env::var("VDF_T_DEFAULT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1 << 20);
    let poll_secs = env::var("RELAYER_POLL_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(8);
    let startup_lookback_blocks = env::var("RELAYER_START_LOOKBACK_BLOCKS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(500);

    let source_provider: Arc<EthProvider> = Arc::new(
        Provider::<Http>::try_from(sepolia_rpc)
            .context("failed creating sepolia provider")?
            .interval(Duration::from_millis(1500)),
    );
    let destination_provider = Provider::<Http>::try_from(destination_rpc)
        .context("failed creating destination provider")?
        .interval(Duration::from_millis(1500));

    let expected_destination_chain_id = env::var("DEST_CHAIN_ID")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(80002);

    let chain_id = destination_provider
        .get_chainid()
        .await
        .context("failed to fetch destination chain id")?
        .as_u64();
    if chain_id != expected_destination_chain_id {
        anyhow::bail!(
            "destination chain id mismatch: expected {}, got {}",
            expected_destination_chain_id,
            chain_id
        );
    }
    let wallet: Wallet<k256::ecdsa::SigningKey> = private_key
        .parse::<LocalWallet>()
        .context("failed parsing PRIVATE_KEY")?
        .with_chain_id(chain_id);

    let signer: Arc<WalletSigner> = Arc::new(SignerMiddleware::new(destination_provider, wallet));

    let mut processed_requests: HashSet<u64> = HashSet::new();
    let mut from_block = current_block(&source_provider)
        .await?
        .saturating_sub(startup_lookback_blocks);

    tracing::info!(
        "Relayer started: sender={}, receiver={}, from_block={}, poll={}s, t={}, lookback_blocks={}",
        sender_addr,
        receiver_addr,
        from_block,
        poll_secs,
        vdf_t,
        startup_lookback_blocks
    );

    loop {
        let latest = current_block(&source_provider).await?;

        if latest >= from_block {
            let events = fetch_log_requests_in_range(
                source_provider.clone(),
                sender_addr,
                from_block,
                latest,
            )
            .await?;

            for ev in events {
                let request_id = ev.request_id.as_u64();
                if processed_requests.contains(&request_id) {
                    continue;
                }

                let session_id = format!("sepolia-req-{}", request_id);
                let pipeline = run_randomness_pipeline_with_seed(&session_id, &ev.seed_user, vdf_t)
                    .with_context(|| format!("pipeline failed for request_id={request_id}"))?;

                match relay_payload_to_receiver(
                    signer.clone(),
                    receiver_addr,
                    RelayPayload {
                        request_id,
                        y: pipeline.payload.y,
                        pi: pipeline.payload.pi,
                        seed_collective: pipeline.metadata.seed_collective.to_vec(),
                        aggregate_signature: pipeline.payload.aggregate_signature,
                    },
                )
                .await
                {
                    Ok(tx_hash) => {
                        tracing::info!("Relayed request_id={} to destination tx={:?}", request_id, tx_hash);
                        processed_requests.insert(request_id);
                    }
                    Err(error) => {
                        tracing::error!("relay failed for request_id={}: {:?}", request_id, error);
                    }
                }
            }

            from_block = latest.saturating_add(1);
        }

        tokio::time::sleep(Duration::from_secs(poll_secs)).await;
    }
}
