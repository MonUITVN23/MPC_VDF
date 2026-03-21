use std::{collections::HashSet, env, str::FromStr, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use crypto_engine::run_randomness_pipeline_with_seed;
use dotenvy::dotenv;
use ethers::prelude::*;

use network_module::axelar::{relay_payload_to_sender, RelayPayload, WalletSigner};
use network_module::rpc::{current_block, fetch_log_requests_in_range, EthProvider};

fn env_required(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("missing required env var: {name}"))
}

fn parse_address(name: &str, value: &str) -> Result<Address> {
    Address::from_str(value).with_context(|| format!("invalid address in {name}: {value}"))
}

fn hex_env_to_bytes(name: &str) -> Result<Vec<u8>> {
    let value = env_required(name)?;
    let trimmed = value.strip_prefix("0x").unwrap_or(&value);
    hex::decode(trimmed).with_context(|| format!("invalid hex in env var: {name}"))
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let sepolia_rpc = env_required("SEPOLIA_RPC_URL")?;
    let private_key = env_required("PRIVATE_KEY")?;
    let sender_addr = parse_address("RANDOM_SENDER_ADDRESS", &env_required("RANDOM_SENDER_ADDRESS")?)?;
    let vdf_modulus = hex_env_to_bytes("VDF_MODULUS_HEX")?;
    let axelar_native_gas_fee_wei = env::var("AXELAR_NATIVE_GAS_FEE_WEI")
        .ok()
        .and_then(|v| U256::from_dec_str(&v).ok())
        .unwrap_or_else(|| U256::from(1_000_000_000_000_000u64));
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
    let vdf_modulus_hex_for_script = env::var("VDF_MODULUS_HEX").unwrap_or_else(|_| "0x".to_owned());

    let source_provider: Arc<EthProvider> = Arc::new(
        Provider::<Http>::try_from(sepolia_rpc)
            .context("failed creating sepolia provider")?
            .interval(Duration::from_millis(1500)),
    );

    let chain_id = source_provider
        .get_chainid()
        .await
        .context("failed to fetch sepolia chain id")?
        .as_u64();
    if chain_id != 11155111 {
        anyhow::bail!(
            "source chain id mismatch: expected 11155111 (sepolia), got {}",
            chain_id
        );
    }
    let wallet: Wallet<k256::ecdsa::SigningKey> = private_key
        .parse::<LocalWallet>()
        .context("failed parsing PRIVATE_KEY")?
        .with_chain_id(chain_id);

    let signer: Arc<WalletSigner> = Arc::new(SignerMiddleware::new((*source_provider).clone(), wallet));

    let mut processed_requests: HashSet<u64> = HashSet::new();
    let mut from_block = current_block(&source_provider)
        .await?
        .saturating_sub(startup_lookback_blocks);

    tracing::info!(
        "Relayer started: sender={}, from_block={}, poll={}s, t={}, lookback_blocks={}",
        sender_addr,
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
                let mut seed_user = [0u8; 32];
                ev.user_seed.to_big_endian(&mut seed_user);
                let pipeline = run_randomness_pipeline_with_seed(&session_id, &seed_user, vdf_t)
                    .with_context(|| format!("pipeline failed for request_id={request_id}"))?;

                let y_hex = format!("0x{}", hex::encode(&pipeline.payload.y));
                let pi_hex = format!("0x{}", hex::encode(&pipeline.payload.pi));
                let seed_collective_hex = format!("0x{}", hex::encode(pipeline.metadata.seed_collective));
                let bls_signature_hex = format!("0x{}", hex::encode(&pipeline.payload.aggregate_signature));

                println!("\n# ===== COPY-PASTE FOR TESTNET SCRIPT =====");
                println!("export VDF_Y_HEX={}", y_hex);
                println!("export VDF_PI_HEX={}", pi_hex);
                println!("export SEED_COLLECTIVE_HEX={}", seed_collective_hex);
                println!("export VDF_MODULUS_HEX={}", vdf_modulus_hex_for_script);
                println!("export BLS_SIGNATURE_HEX={}", bls_signature_hex);
                println!("# t2_mpc_ms={}", pipeline.metadata.benchmark.t2_mpc_ms);
                println!("# t3_vdf_ms={}", pipeline.metadata.benchmark.t3_vdf_ms);
                println!("# =========================================\n");

                match relay_payload_to_sender(
                    signer.clone(),
                    sender_addr,
                    RelayPayload {
                        request_id,
                        y: pipeline.payload.y,
                        pi: pipeline.payload.pi,
                        seed_collective: pipeline.metadata.seed_collective.to_vec(),
                        modulus: vdf_modulus.clone(),
                        aggregate_signature: pipeline.payload.aggregate_signature,
                        axelar_native_gas_fee_wei,
                    },
                )
                .await
                {
                    Ok(tx_hash) => {
                        println!("+--------------------------------------------------+");
                        println!("| E2E RELAY RESULT                                 |");
                        println!("+--------------------------------------------------+");
                        println!("| request_id : {:<35}|", request_id);
                        println!("| t2_mpc_ms  : {:<35}|", pipeline.metadata.benchmark.t2_mpc_ms);
                        println!("| t3_vdf_ms  : {:<35}|", pipeline.metadata.benchmark.t3_vdf_ms);
                        println!("| tx_hash    : {:<35}|", format!("{tx_hash:?}"));
                        println!("+--------------------------------------------------+");
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
