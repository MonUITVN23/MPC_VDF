pub mod vdf;
pub mod mpc;
pub mod dkg;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Instant;
use std::sync::OnceLock;
use ethers::types::U256;

use halo2_prover::{
    BlsCommitmentInput, generate_keys, generate_params, prove as halo2_prove,
};
use halo2_proofs::{
    pasta::EqAffine,
    plonk::ProvingKey,
    poly::commitment::Params,
};


pub const DEFAULT_TOTAL_NODES: usize = 4;
pub const DEFAULT_THRESHOLD: usize = 3;
pub const STRESS_TOTAL_NODES: usize = 10;
pub const STRESS_THRESHOLD: usize = 7;


fn get_k_parameter() -> u32 {
    std::env::var("HALO2_K")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(halo2_prover::K) 
}

struct Halo2Setup {
    params: Params<EqAffine>,
    pk: ProvingKey<EqAffine>,
}

static HALO2_SETUP: OnceLock<Halo2Setup> = OnceLock::new();

fn get_halo2_setup() -> &'static Halo2Setup {
    HALO2_SETUP.get_or_init(|| {
        let k = get_k_parameter();

        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            for line in meminfo.lines().take(3) {
                eprintln!("  [sys] {}", line);
            }
        }

        eprintln!("INFO: Initializing Halo2 IPA (K={}, 2^{}={} rows, no trusted setup)...", k, k, 1u64 << k);
        let start = Instant::now();

        let params = generate_params(k);
        let (pk, _vk) = generate_keys(&params).expect("failed to generate Halo2 keys");

        let setup_ms = start.elapsed().as_millis();
        eprintln!("INFO: Halo2 setup complete in {} ms (fully transparent, no ceremony).", setup_ms);

        Halo2Setup { params, pk }
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RandomnessExport {
	pub y: Vec<u8>,
	pub pi: Vec<u8>,
	pub aggregate_signature: Vec<u8>,
	pub zk_proof_data: Vec<u8>,
	pub zk_public_signals: [U256; 7],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineMetadata {
	pub session_id: String,
	pub seed_collective: [u8; 32],
	pub benchmark: BenchmarkMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetrics {
	pub t2_mpc_ms: u128,
	pub t3_vdf_ms: u128,
	pub t3_5_zkprove_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineOutput {
	pub metadata: PipelineMetadata,
	pub payload: RandomnessExport,
}

pub fn run_randomness_pipeline_full(
	session_id: &str,
	seed_user: &[u8],
	t: u64,
	request_id: u64,
	modulus: &[u8],
) -> Result<PipelineOutput> {
	let start_t2 = Instant::now();
	let collective = mpc::init_collective_seed_default()?;

	let mut hasher = Sha256::new();
	hasher.update(b"mpc-vdf-session");
	hasher.update(session_id.as_bytes());
	hasher.update(seed_user);
	hasher.update(collective.seed_collective);
	let seed_collective: [u8; 32] = hasher.finalize().into();
	let t2_mpc_ms = start_t2.elapsed().as_millis();

	let start_t3 = Instant::now();
	let vdf_output = vdf::evaluate_and_generate_proof(&seed_collective, t)?;
	let t3_vdf_ms = start_t3.elapsed().as_millis();

    let start_zk = Instant::now();
    let (zk_proof_data, zk_public_signals) = generate_zk_proof(
        &collective.aggregate_signature,
        &collective.aggregated_public_key,
        &seed_collective,
        &vdf_output.y_bytes,
        &vdf_output.proof_pi_bytes,
        modulus,
        request_id,
    ).unwrap_or_else(|e| {
        eprintln!("Warning: ZK proof generation failed: {:?}", e);
        (Vec::new(), [U256::zero(); 7])
    });
    let t3_5_zkprove_ms = start_zk.elapsed().as_millis();

	Ok(PipelineOutput {
		metadata: PipelineMetadata {
			session_id: session_id.to_owned(),
			seed_collective,
			benchmark: BenchmarkMetrics { t2_mpc_ms, t3_vdf_ms, t3_5_zkprove_ms },
		},
		payload: RandomnessExport {
			y: vdf_output.y_bytes,
			pi: vdf_output.proof_pi_bytes,
			aggregate_signature: collective.aggregate_signature,
            zk_proof_data,
            zk_public_signals,
		},
	})
}

pub fn run_randomness_pipeline_with_seed(
	session_id: &str,
	seed_user: &[u8],
	t: u64,
) -> Result<PipelineOutput> {
	run_randomness_pipeline_full(session_id, seed_user, t, 0, &[0u8; 32])
}

fn generate_zk_proof(
    sig: &[u8],
    pk: &[u8],
    msg: &[u8],
    y: &[u8],
    pi: &[u8],
    modulus: &[u8],
    request_id: u64,
) -> Result<(Vec<u8>, [U256; 7])> {
    let input = BlsCommitmentInput::from_raw(pk, sig, msg, y, pi, modulus, request_id);
    let setup = get_halo2_setup();

    let result = halo2_prove(&setup.params, &setup.pk, &input)
        .context("Halo2 proof generation failed")?;

    eprintln!(
        "INFO: Halo2 proof generated in {} ms (proof: {} bytes, K={})",
        result.proving_time_ms, result.proof_bytes.len(), get_k_parameter()
    );

    let mut zk_public_signals = [U256::zero(); 7];
    for (i, sig_str) in result.public_signals.iter().enumerate() {
        zk_public_signals[i] = U256::from_dec_str(sig_str)
            .with_context(|| format!("failed to parse signal {}: {}", i, sig_str))?;
    }

    let token = ethers::abi::Token::Bytes(result.proof_bytes);
    let zk_proof_data = ethers::abi::encode(&[token]);

    Ok((zk_proof_data, zk_public_signals))
}

pub fn bench_zk_only(t: u64) -> u128 {
    let collective = mpc::init_collective_seed_default().unwrap();
    let seed_collective = collective.seed_collective;
    
    let vdf_output = vdf::evaluate_and_generate_proof(&seed_collective, t).unwrap();

    let mut modulus = [0u8; 32];
    modulus[31] = 17; 
    let start = Instant::now();

    let _ = generate_zk_proof(
        &collective.aggregate_signature,
        &collective.aggregated_public_key,
        &seed_collective,
        &vdf_output.y_bytes,
        &vdf_output.proof_pi_bytes,
        &modulus,
        1,
    );

    start.elapsed().as_millis()
}

pub fn run_randomness_pipeline(session_id: &str, t: u64) -> Result<PipelineOutput> {
	run_randomness_pipeline_with_seed(session_id, &[], t)
}

pub fn run_randomness_pipeline_json(session_id: &str, t: u64) -> Result<String> {
	let output = run_randomness_pipeline(session_id, t)?;
	Ok(serde_json::to_string_pretty(&output)?)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn pipeline_exports_required_fields() {
		let output = run_randomness_pipeline("session-local-001", 64).unwrap();
		assert!(!output.payload.y.is_empty());
		assert!(!output.payload.pi.is_empty());
		assert!(!output.payload.aggregate_signature.is_empty());
	}

	#[test]
	fn pipeline_with_seed_user_exports_required_fields() {
		let output = run_randomness_pipeline_with_seed("session-local-002", b"seed_user", 64).unwrap();
		assert!(!output.payload.y.is_empty());
		assert!(!output.payload.pi.is_empty());
		assert!(!output.payload.aggregate_signature.is_empty());
	}

	#[test]
	fn pipeline_generates_halo2_proof() {
		let output = run_randomness_pipeline_full(
			"session-zk-test", b"test_seed", 64, 42, &[0xAB; 32],
		).unwrap();

		assert!(!output.payload.zk_proof_data.is_empty(), "ZK proof should not be empty");
		assert!(output.metadata.benchmark.t3_5_zkprove_ms > 0, "ZK time should be recorded");
		assert_ne!(output.payload.zk_public_signals[0], U256::zero(), "commitment_hi non-zero");
		assert_eq!(output.payload.zk_public_signals[6], U256::from(42u64), "request_id = 42");
	}
}
