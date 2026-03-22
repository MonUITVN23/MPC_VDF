use std::{
    collections::HashSet,
    env,
    fs::OpenOptions,
    io::Write,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use crypto_engine::run_randomness_pipeline_with_seed;
use dotenvy::dotenv;
use ethers::prelude::*;

use network_module::bridges::{
    AxelarRelayer, LayerZeroMockRelayer, MultiBridgeRouter, RelayPayload, WalletSigner,
};
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

fn optional_hex_env_to_bytes(name: &str) -> Result<Vec<u8>> {
    match env::var(name) {
        Ok(value) => {
            let trimmed = value.strip_prefix("0x").unwrap_or(&value);
            hex::decode(trimmed).with_context(|| format!("invalid hex in env var: {name}"))
        }
        Err(_) => Ok(Vec::new()),
    }
}

fn append_metrics_csv(
    file_path: &str,
    request_id: u64,
    bridge_id: u8,
    t1_timestamp: u64,
    t2_mpc_ms: u128,
    t3_vdf_ms: u128,
    t4_dispatch_ms: u128,
    tx_hash: H256,
) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)
        .with_context(|| format!("failed to open metrics csv: {file_path}"))?;

    let is_empty = file
        .metadata()
        .with_context(|| format!("failed to read metadata for: {file_path}"))?
        .len()
        == 0;

    if is_empty {
        writeln!(
            file,
            "request_id,bridge_id,t1_timestamp,t2_mpc_ms,t3_vdf_ms,t4_dispatch_ms,tx_hash"
        )
            .with_context(|| format!("failed writing header to csv: {file_path}"))?;
    }

    writeln!(
        file,
        "{request_id},{bridge_id},{t1_timestamp},{t2_mpc_ms},{t3_vdf_ms},{t4_dispatch_ms},{:#x}",
        tx_hash
    )
    .with_context(|| format!("failed appending metrics row to csv: {file_path}"))?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let sepolia_rpc = env_required("SEPOLIA_RPC_URL")?;
    let private_key = env_required("PRIVATE_KEY")?;
    let sender_addr = parse_address("RANDOM_SENDER_ADDRESS", &env_required("RANDOM_SENDER_ADDRESS")?)?;
    let axelar_gas_service_addr =
        parse_address("AXELAR_GAS_SERVICE_ADDRESS", &env_required("AXELAR_GAS_SERVICE_ADDRESS")?)?;
    let vdf_modulus = hex_env_to_bytes("VDF_MODULUS_HEX")?;
    let cross_chain_fee_wei = env::var("CROSS_CHAIN_FEE_WEI")
        .ok()
        .or_else(|| env::var("AXELAR_NATIVE_GAS_FEE_WEI").ok())
        .and_then(|v| U256::from_dec_str(&v).ok())
        .unwrap_or_else(|| U256::from(200_000_000_000_000u64));
    let cross_chain_fee_cap_wei = env::var("CROSS_CHAIN_FEE_CAP_WEI")
        .ok()
        .and_then(|v| U256::from_dec_str(&v).ok())
        .unwrap_or_else(|| U256::from(500_000_000_000_000u64));
    let cross_chain_daily_budget_wei = env::var("CROSS_CHAIN_DAILY_BUDGET_WEI")
        .ok()
        .and_then(|v| U256::from_dec_str(&v).ok())
        .unwrap_or_else(|| U256::from(5_000_000_000_000_000u64));
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
    let axelar_destination_chain =
        env::var("AXELAR_DESTINATION_CHAIN").unwrap_or_else(|_| "polygon-sepolia".to_owned());
    let axelar_destination_address = env::var("AXELAR_DESTINATION_ADDRESS")
        .ok()
        .or_else(|| env::var("RANDOM_RECEIVER_ADDRESS").ok())
        .unwrap_or_default();
    let axelar_execution_gas_limit = env::var("AXELAR_EXECUTION_GAS_LIMIT")
        .ok()
        .and_then(|v| U256::from_dec_str(&v).ok())
        .unwrap_or_else(|| U256::from(700_000u64));
    let axelar_estimate_params = optional_hex_env_to_bytes("AXELAR_ESTIMATE_PARAMS_HEX")?;
    let axelar_fee_buffer_bps = env::var("AXELAR_FEE_BUFFER_BPS")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| *v >= 11_000 && *v <= 12_000)
        .unwrap_or(12_000);

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
    let axelar_relayer = AxelarRelayer::new(
        signer.clone(),
        sender_addr,
        axelar_gas_service_addr,
        axelar_destination_chain,
        axelar_destination_address,
        axelar_execution_gas_limit,
        axelar_estimate_params.into(),
        axelar_fee_buffer_bps,
        cross_chain_fee_cap_wei,
        cross_chain_daily_budget_wei,
    );
    let layerzero_mock_relayer = LayerZeroMockRelayer::new();
    let router = MultiBridgeRouter::default_with_priority(axelar_relayer, layerzero_mock_relayer);

    let mut processed_requests: HashSet<u64> = HashSet::new();
    let mut from_block = current_block(&source_provider)
        .await?
        .saturating_sub(startup_lookback_blocks);
    let mut last_seen_block = 0u64;

    tracing::info!(
        "Relayer started: sender={}, from_block={}, poll={}s, t={}, lookback_blocks={}",
        sender_addr,
        from_block,
        poll_secs,
        vdf_t,
        startup_lookback_blocks
    );
    tracing::info!(
        "Bắt đầu lắng nghe tại địa chỉ {} (mode=HTTP polling, interval={}s)",
        sender_addr,
        poll_secs
    );

    loop {
        let latest = current_block(&source_provider).await?;
        if latest != last_seen_block {
            tracing::info!(
                "Đã nhận được block mới: latest={}, scanning_range={}..{}",
                latest,
                from_block,
                latest
            );
            last_seen_block = latest;
        }

        if latest >= from_block {
            let events = fetch_log_requests_in_range(
                source_provider.clone(),
                sender_addr,
                from_block,
                latest,
            )
            .await?;

            tracing::info!(
                "Scan LogRequest xong: from_block={}, to_block={}, events_found={}",
                from_block,
                latest,
                events.len()
            );

            for ev in events {
                let request_id = ev.request_id.as_u64();
                let t1_timestamp = ev.timestamp.as_u64();
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

                let dispatch_started = Instant::now();

                match router
                    .execute_with_failover(RelayPayload {
                        request_id,
                        bridge_id: 0,
                        y: pipeline.payload.y,
                        pi: pipeline.payload.pi,
                        seed_collective: pipeline.metadata.seed_collective.to_vec(),
                        modulus: vdf_modulus.clone(),
                        aggregate_signature: pipeline.payload.aggregate_signature,
                        cross_chain_fee_wei,
                    })
                    .await
                {
                    Ok((tx_hash, bridge_id)) => {
                        let source_tx_dispatch_ms = dispatch_started.elapsed().as_millis();
                        tracing::info!(
                            request_id,
                            bridge_id,
                            source_tx_dispatch_ms,
                            "source chain dispatch finished"
                        );

                        append_metrics_csv(
                            "e2e_metrics.csv",
                            request_id,
                            bridge_id,
                            t1_timestamp,
                            pipeline.metadata.benchmark.t2_mpc_ms,
                            pipeline.metadata.benchmark.t3_vdf_ms,
                            source_tx_dispatch_ms,
                            tx_hash,
                        )?;

                        println!("+--------------------------------------------------+");
                        println!("| E2E RELAY RESULT                                 |");
                        println!("+--------------------------------------------------+");
                        println!("| request_id : {:<35}|", request_id);
                        println!("| bridge_id  : {:<35}|", bridge_id);
                        println!("| t2_mpc_ms  : {:<35}|", pipeline.metadata.benchmark.t2_mpc_ms);
                        println!("| t3_vdf_ms  : {:<35}|", pipeline.metadata.benchmark.t3_vdf_ms);
                        println!("| t4_src_ms  : {:<35}|", source_tx_dispatch_ms);
                        println!("| tx_hash    : {:<35}|", format!("{tx_hash:?}"));
                        println!("+--------------------------------------------------+");
                        processed_requests.insert(request_id);
                    }
                    Err(error) => {
                        let source_tx_dispatch_ms = dispatch_started.elapsed().as_millis();
                        tracing::warn!(
                            request_id,
                            source_tx_dispatch_ms,
                            "source chain dispatch failed after retries"
                        );
                        tracing::error!("relay failed for request_id={}: {:?}", request_id, error);
                    }
                }
            }

            from_block = latest.saturating_add(1);
        }

        tokio::time::sleep(Duration::from_secs(poll_secs)).await;
    }
}
