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
use crypto_engine::run_randomness_pipeline_full;
use dotenvy::dotenv;
use ethers::prelude::*;

use network_module::bridge_registry::resolve_bridge_priority;
use network_module::bridges::{BridgeMetadata, BridgeRelayer, MultiBridgeRouter, RelayPayload, WalletSigner};
use network_module::relayer_factory::{build_builtin_relayers, RelayerFactoryInput};
use network_module::rpc::{current_block, fetch_log_requests_in_range, EthProvider};

fn csv_escape(value: &str) -> String {
    let escaped = value.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

fn env_required(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("missing required env var: {name}"))
}

fn parse_address(name: &str, value: &str) -> Result<Address> {
    Address::from_str(value).with_context(|| format!("invalid address in {name}: {value}"))
}

fn env_required_one_of(primary: &str, fallback: &str) -> Result<String> {
    env::var(primary)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| env::var(fallback).ok().filter(|v| !v.trim().is_empty()))
        .with_context(|| format!("missing required env var: {primary} (or fallback {fallback})"))
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

fn append_metrics_csv_v2(
    file_path: &str,
    request_id: u64,
    t1_timestamp: u64,
    t2_mpc_ms: u128,
    t3_vdf_ms: u128,
    t4_dispatch_ms: u128,
    bridge_name: &str,
    bridge_id_hex: &str,
    selected_bridge: &str,
    attempt_count: u8,
    fallback_hops: u8,
    dispatch_status: &str,
    error_reason: &str,
    tx_hash: Option<H256>,
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
            "request_id,t1_timestamp,t2_mpc_ms,t3_vdf_ms,t4_dispatch_ms,bridge_name,bridge_id_hex,selected_bridge,attempt_count,fallback_hops,dispatch_status,error_reason,tx_hash"
        )
            .with_context(|| format!("failed writing header to csv: {file_path}"))?;
    }

    let tx_hash_text = tx_hash
        .map(|hash| format!("{:#x}", hash))
        .unwrap_or_default();

    writeln!(
        file,
        "{request_id},{t1_timestamp},{t2_mpc_ms},{t3_vdf_ms},{t4_dispatch_ms},{},{},{},{attempt_count},{fallback_hops},{},{},{}",
        csv_escape(bridge_name),
        csv_escape(bridge_id_hex),
        csv_escape(selected_bridge),
        csv_escape(dispatch_status),
        csv_escape(error_reason),
        csv_escape(&tx_hash_text),
    )
    .with_context(|| format!("failed appending metrics row to csv: {file_path}"))?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("DEBUG: Starting network_module main function");
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let sepolia_rpc = env_required("SEPOLIA_RPC_URL")?;
    let private_key = env_required("PRIVATE_KEY")?;
    let router_addr = parse_address(
        "RANDOM_ROUTER_ADDRESS",
        &env_required_one_of("RANDOM_ROUTER_ADDRESS", "RANDOM_SENDER_ADDRESS")?,
    )?;
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
    let layerzero_fee_buffer_bps = env::var("LAYERZERO_FEE_BUFFER_BPS")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| *v >= 10_000 && *v <= 13_000)
        .unwrap_or(11_000);
    let wormhole_fee_buffer_bps = env::var("WORMHOLE_FEE_BUFFER_BPS")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| *v >= 10_000 && *v <= 13_000)
        .unwrap_or(11_000);
    let bridge_priority = resolve_bridge_priority()?;
    let metrics_file_path = env::var("E2E_METRICS_V2_PATH")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "e2e_metrics_v2.csv".to_owned());
    let bridge_timeout_secs = env::var("RELAYER_BRIDGE_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(90);

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
    let mut available_relayers = build_builtin_relayers(RelayerFactoryInput {
        signer: signer.clone(),
        router_address: router_addr,
        axelar_gas_service_address: axelar_gas_service_addr,
        axelar_destination_chain,
        axelar_destination_address,
        axelar_execution_gas_limit,
        axelar_estimate_params: axelar_estimate_params.into(),
        axelar_fee_buffer_bps,
        layerzero_fee_buffer_bps,
        wormhole_fee_buffer_bps,
        cross_chain_fee_cap_wei,
        cross_chain_daily_budget_wei,
    });

    let mut relayers_with_metadata: Vec<(BridgeMetadata, Box<dyn BridgeRelayer + Send + Sync>)> = Vec::new();
    for bridge_name in &bridge_priority.names {
        match available_relayers.remove(bridge_name.as_str()) {
            Some(relayer) => {
                relayers_with_metadata.push((BridgeMetadata::from_name(bridge_name.as_str()), relayer));
            }
            None => {
                tracing::warn!(bridge_name = %bridge_name, "bridge in BRIDGE_PRIORITY is unknown or duplicated; skipping");
            }
        }
    }

    if relayers_with_metadata.is_empty() {
        anyhow::bail!(
            "no valid bridge in BRIDGE_PRIORITY='{}'; supported bridges: AXELAR,LAYERZERO,WORMHOLE",
            bridge_priority.raw
        );
    }

    let router = MultiBridgeRouter::with_timeout(
        relayers_with_metadata,
        Duration::from_secs(bridge_timeout_secs),
    );

    let mut processed_requests: HashSet<u64> = HashSet::new();
    let mut from_block = current_block(&source_provider)
        .await?
        .saturating_sub(startup_lookback_blocks);
    let mut last_seen_block = 0u64;

    tracing::info!(
        "Relayer started: router={}, from_block={}, poll={}s, t={}, lookback_blocks={}",
        router_addr,
        from_block,
        poll_secs,
        vdf_t,
        startup_lookback_blocks
    );
    tracing::info!(bridge_priority = %bridge_priority.raw, source = %bridge_priority.source, "bridge failover priority loaded");
    tracing::info!(bridge_timeout_secs, "per-bridge relay timeout configured");
    tracing::info!(
        "Bắt đầu lắng nghe tại địa chỉ {} (mode=HTTP polling, interval={}s)",
        router_addr,
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
                router_addr,
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
                let pipeline = run_randomness_pipeline_full(&session_id, &seed_user, vdf_t, request_id, &vdf_modulus)
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
                        zk_proof_data: pipeline.payload.zk_proof_data,
                        zk_public_signals: pipeline.payload.zk_public_signals,
                    })
                    .await
                {
                    Ok(dispatch_result) => {
                        let source_tx_dispatch_ms = dispatch_started.elapsed().as_millis();
                        let fallback_hops = dispatch_result.attempt_count.saturating_sub(1);
                        tracing::info!(
                            request_id,
                            bridge_name = %dispatch_result.bridge_name,
                            bridge_id_hex = %dispatch_result.bridge_id_hex,
                            attempt_count = dispatch_result.attempt_count,
                            source_tx_dispatch_ms,
                            "source chain dispatch finished"
                        );

                        append_metrics_csv_v2(
                            &metrics_file_path,
                            request_id,
                            t1_timestamp,
                            pipeline.metadata.benchmark.t2_mpc_ms,
                            pipeline.metadata.benchmark.t3_vdf_ms,
                            source_tx_dispatch_ms,
                            &dispatch_result.bridge_name,
                            &dispatch_result.bridge_id_hex,
                            &dispatch_result.bridge_name,
                            dispatch_result.attempt_count,
                            fallback_hops,
                            "success",
                            "",
                            Some(dispatch_result.tx_hash),
                        )?;

                        println!("+--------------------------------------------------+");
                        println!("| E2E RELAY RESULT                                 |");
                        println!("+--------------------------------------------------+");
                        println!("| request_id : {:<35}|", request_id);
                        println!("| bridge     : {:<35}|", dispatch_result.bridge_name);
                        println!("| bridge_hex : {:<35}|", dispatch_result.bridge_id_hex);
                        println!("| attempts   : {:<35}|", dispatch_result.attempt_count);
                        println!("| t2_mpc_ms  : {:<35}|", pipeline.metadata.benchmark.t2_mpc_ms);
                        println!("| t3_vdf_ms  : {:<35}|", pipeline.metadata.benchmark.t3_vdf_ms);
                        println!("| t4_src_ms  : {:<35}|", source_tx_dispatch_ms);
                        println!("| tx_hash    : {:<35}|", format!("{:?}", dispatch_result.tx_hash));
                        println!("+--------------------------------------------------+");
                        processed_requests.insert(request_id);
                    }
                    Err(error) => {
                        let source_tx_dispatch_ms = dispatch_started.elapsed().as_millis();
                        let attempt_count = u8::try_from(router.relayer_count()).unwrap_or(u8::MAX);
                        let fallback_hops = attempt_count.saturating_sub(1);
                        tracing::warn!(
                            request_id,
                            attempt_count,
                            source_tx_dispatch_ms,
                            "source chain dispatch failed after retries"
                        );
                        tracing::error!("relay failed for request_id={}: {:?}", request_id, error);

                        append_metrics_csv_v2(
                            &metrics_file_path,
                            request_id,
                            t1_timestamp,
                            pipeline.metadata.benchmark.t2_mpc_ms,
                            pipeline.metadata.benchmark.t3_vdf_ms,
                            source_tx_dispatch_ms,
                            "",
                            "",
                            "NONE",
                            attempt_count,
                            fallback_hops,
                            "failed",
                            &error.to_string(),
                            None,
                        )?;
                    }
                }
            }

            from_block = latest.saturating_add(1);
        }

        tokio::time::sleep(Duration::from_secs(poll_secs)).await;
    }
}
