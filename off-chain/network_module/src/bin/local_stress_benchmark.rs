use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use crypto_engine::run_randomness_pipeline_with_seed;
use ethers::{
    abi::Abi,
    prelude::*,
    types::transaction::eip2718::TypedTransaction,
    utils::Anvil,
};
use serde::Deserialize;

abigen!(
    VdfVerifierMock,
    r#"[
        function verifyVDFPublic(bytes base, bytes exponent, bytes modulus) view returns (bytes)
    ]"#
);

type LocalWalletSigner = SignerMiddleware<Provider<Http>, Wallet<k256::ecdsa::SigningKey>>;

#[derive(Debug, Deserialize)]
struct HardhatArtifact {
    abi: Abi,
    bytecode: String,
}

fn env_or_default(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}

fn parse_t_values() -> Result<Vec<u64>> {
    let raw = env_or_default("BENCH_T_VALUES", "32768,262144,1048576,4194304");
    let mut out = Vec::new();

    for item in raw.split(',') {
        let value = item.trim();
        if value.is_empty() {
            continue;
        }
        out.push(
            value
                .parse::<u64>()
                .with_context(|| format!("invalid T value: {value}"))?,
        );
    }

    if out.is_empty() {
        anyhow::bail!("BENCH_T_VALUES parsed empty");
    }

    Ok(out)
}

fn parse_modulus_bytes() -> Result<Vec<u8>> {
    if let Ok(value) = env::var("VDF_MODULUS_HEX") {
        let trimmed = value.strip_prefix("0x").unwrap_or(&value);
        return hex::decode(trimmed).context("failed to decode VDF_MODULUS_HEX");
    }

    let mut modulus = vec![0x11u8; 130];
    let last_idx = modulus.len() - 1;
    modulus[last_idx] = 1;
    Ok(modulus)
}

fn build_seed_user(session_id: &str, t_value: u64, run_idx: u32) -> [u8; 32] {
    let input = format!("{session_id}:{t_value}:{run_idx}");
    ethers::utils::keccak256(input).into()
}

fn load_artifact_path() -> PathBuf {
    let default = "../contracts/artifacts/src/mock/VDFVerifierMock.sol/VDFVerifierMock.json";
    PathBuf::from(env_or_default("VDF_MOCK_ARTIFACT_PATH", default))
}

fn load_hardhat_artifact(path: &Path) -> Result<HardhatArtifact> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed reading artifact file: {}", path.display()))?;
    let artifact: HardhatArtifact =
        serde_json::from_str(&content).context("failed parsing artifact json")?;
    Ok(artifact)
}

async fn deploy_verifier(client: Arc<LocalWalletSigner>) -> Result<VdfVerifierMock<LocalWalletSigner>> {
    let artifact_path = load_artifact_path();
    let artifact = load_hardhat_artifact(&artifact_path)?;
    let bytecode: Bytes = artifact
        .bytecode
        .parse()
        .context("failed parsing artifact bytecode")?;

    let factory = ContractFactory::new(artifact.abi, bytecode, client.clone());
    let deployer = factory
        .deploy(())
        .context("failed creating deployer for VDFVerifierMock")?;
    let contract = deployer.send().await.context("failed deploying VDFVerifierMock")?;

    Ok(VdfVerifierMock::new(contract.address(), client))
}

fn append_csv_header_if_needed(file_path: &Path) -> Result<()> {
    let exists_and_non_empty = file_path.exists() && fs::metadata(file_path)?.len() > 0;
    if exists_and_non_empty {
        return Ok(());
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)
        .with_context(|| format!("failed opening csv file: {}", file_path.display()))?;

    writeln!(file, "T_value,prover_time_ms,proof_size_bytes,verify_gas_used")
        .context("failed writing csv header")?;
    Ok(())
}

fn append_csv_row(
    file_path: &Path,
    t_value: u64,
    prover_time_ms: u128,
    proof_size_bytes: usize,
    verify_gas_used: U256,
) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)
        .with_context(|| format!("failed opening csv file: {}", file_path.display()))?;

    writeln!(
        file,
        "{t_value},{prover_time_ms},{proof_size_bytes},{}",
        verify_gas_used
    )
    .context("failed writing csv row")?;
    Ok(())
}

async fn measure_verify_gas(
    verifier: &VdfVerifierMock<LocalWalletSigner>,
    base: Vec<u8>,
    exponent: Vec<u8>,
    modulus: Vec<u8>,
) -> Result<U256> {
    let call = verifier.verify_vdf_public(base.into(), exponent.into(), modulus.into());
    let calldata = call
        .calldata()
        .ok_or_else(|| anyhow::anyhow!("failed encoding verifyVDFPublic calldata"))?;

    let tx: TypedTransaction = Eip1559TransactionRequest {
        to: Some(NameOrAddress::Address(verifier.address())),
        data: Some(calldata),
        gas: Some(U256::from(1_200_000u64)),
        ..Default::default()
    }
    .into();

    let client = verifier.client();
    let pending = client
        .send_transaction(tx, None)
        .await
        .context("failed sending verifyVDFPublic tx")?;

    let receipt = pending
        .await
        .context("failed waiting verifyVDFPublic receipt")?
        .ok_or_else(|| anyhow::anyhow!("verifyVDFPublic tx dropped from mempool"))?;

    if receipt.status != Some(U64::from(1u64)) {
        anyhow::bail!("verifyVDFPublic reverted: tx={:?}", receipt.transaction_hash);
    }

    receipt
        .gas_used
        .ok_or_else(|| anyhow::anyhow!("missing gasUsed in verifyVDFPublic receipt"))
}

#[tokio::main]
async fn main() -> Result<()> {
    let t_values = parse_t_values()?;
    let repeats_per_t = env_or_default("BENCH_REPEATS_PER_T", "5")
        .parse::<u32>()
        .context("invalid BENCH_REPEATS_PER_T")?;
    let csv_path = PathBuf::from(env_or_default(
        "CRYPTO_BENCH_CSV_PATH",
        "crypto_benchmarks.csv",
    ));

    append_csv_header_if_needed(&csv_path)?;

    let anvil = Anvil::new().spawn();
    let provider = Provider::<Http>::try_from(anvil.endpoint())?;
    let chain_id = provider.get_chainid().await?.as_u64();
    let wallet: LocalWallet = anvil.keys()[0].clone().into();
    let wallet = wallet.with_chain_id(chain_id);
    let client: Arc<LocalWalletSigner> = Arc::new(SignerMiddleware::new(provider, wallet));

    let verifier = deploy_verifier(client.clone()).await?;
    let modulus = parse_modulus_bytes()?;

    println!(
        "[local-stress] start -> csv={}, repeats_per_t={}, t_values={:?}",
        csv_path.display(),
        repeats_per_t,
        t_values
    );

    for t_value in t_values {
        for run_idx in 0..repeats_per_t {
            let session_id = format!("local-bench-t{t_value}-run{run_idx}");
            let seed_user = build_seed_user(&session_id, t_value, run_idx);

            let pipeline = run_randomness_pipeline_with_seed(&session_id, &seed_user, t_value)
                .with_context(|| format!("pipeline failed for T={t_value}, run={run_idx}"))?;

            let prover_time_ms = pipeline.metadata.benchmark.t3_vdf_ms;
            let proof_size_bytes = pipeline.payload.y.len() + pipeline.payload.pi.len();

            let verify_gas_used = measure_verify_gas(
                &verifier,
                pipeline.metadata.seed_collective.to_vec(),
                pipeline.payload.pi,
                modulus.clone(),
            )
            .await
            .with_context(|| format!("verify gas measurement failed for T={t_value}, run={run_idx}"))?;

            append_csv_row(
                &csv_path,
                t_value,
                prover_time_ms,
                proof_size_bytes,
                verify_gas_used,
            )?;

            println!(
                "[local-stress] T={} run={} prover_ms={} proof_bytes={} verify_gas={}",
                t_value, run_idx, prover_time_ms, proof_size_bytes, verify_gas_used
            );
        }
    }

    println!("[local-stress] done -> {}", csv_path.display());
    Ok(())
}
