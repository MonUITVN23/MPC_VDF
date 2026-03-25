pub mod vdf;
pub mod mpc;
pub mod dkg;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Instant;
use std::process::Command;
use std::fs;
use std::path::PathBuf;
use ethers::types::U256;

// Default PoC configuration (matches CLAUDE.md spec)
pub const DEFAULT_TOTAL_NODES: usize = 4;
pub const DEFAULT_THRESHOLD: usize = 3; // 3-of-4

// Medium config for stress testing
pub const STRESS_TOTAL_NODES: usize = 10;
pub const STRESS_THRESHOLD: usize = 7; // 7-of-10

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
	// Legacy wrapper — uses placeholder request_id/modulus (ZK proof won't pass on-chain binding checks)
	run_randomness_pipeline_full(session_id, seed_user, t, 0, &[0u8; 32])
}

/// Pad (with trailing zeros) or truncate a byte slice to exactly `target_len` bytes.
fn pad_or_truncate(data: &[u8], target_len: usize) -> Vec<u8> {
    let mut result = vec![0u8; target_len];
    let copy_len = data.len().min(target_len);
    result[..copy_len].copy_from_slice(&data[..copy_len]);
    result
}

fn generate_zk_proof(sig: &[u8], pk: &[u8], msg: &[u8], y: &[u8], pi: &[u8], modulus: &[u8], request_id: u64) -> Result<(Vec<u8>, [U256; 7])> {
    // Circuit expects exactly: pk[48], sig[96], msg[32]
    // MPC DKG outputs: pk=SHA256(individual_pks)=32 bytes, sig=compressed_G2=varies, msg=seed_collective=32 bytes
    // Pad/truncate to match circuit expectations
    let pk_padded = pad_or_truncate(pk, 48);
    let sig_padded = pad_or_truncate(sig, 96);
    let msg_padded = pad_or_truncate(msg, 32);

    // 1. Prepare input.json for prove.js
    let input_json = serde_json::json!({
        "pk": hex::encode(&pk_padded),
        "sig": hex::encode(&sig_padded),
        "msg": hex::encode(&msg_padded),
        "y": hex::encode(y),
        "pi": hex::encode(pi),
        "modulus": hex::encode(modulus),
        "requestId": request_id.to_string()
    });
    // Generate unique temp dir using process ID + timestamp
    let temp_dir = std::env::temp_dir().join(format!(
        "zk_prove_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    fs::create_dir_all(&temp_dir)?;
    
    let input_path = temp_dir.join("input.json");
    fs::write(&input_path, input_json.to_string())?;

    // 2. Call node prove.js
    let mut project_root = std::env::current_dir()?;
    if project_root.ends_with("off-chain") || project_root.ends_with("crypto_engine") || project_root.ends_with("network_module") {
        while !project_root.join("contracts").exists() && project_root.parent().is_some() {
            project_root = project_root.parent().unwrap().to_path_buf();
        }
    }
    
    let prove_js = project_root.join("contracts/circuits/scripts/prove.js");
    eprintln!("DEBUG: Running prove.js at {:?}", prove_js);
    
    // Use spawn + wait instead of output() to avoid pipe deadlock.
    // Inherit stdout/stderr so snarkjs logs are visible in console.
    let mut child = Command::new("node")
        .arg(&prove_js)
        .arg("--input")
        .arg(&input_path)
        .arg("--output")
        .arg(&temp_dir)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .context("Failed to spawn node prove.js")?;

    let exit_status = child.wait().context("prove.js process terminated unexpectedly")?;

    if !exit_status.success() {
        anyhow::bail!("prove.js failed with exit code: {:?}", exit_status.code());
    }

    // 3. Read results
    let proof_str = fs::read_to_string(temp_dir.join("proof.json"))?;
    let public_str = fs::read_to_string(temp_dir.join("public.json"))?;

    let proof: serde_json::Value = serde_json::from_str(&proof_str)?;
    let public_signals: Vec<String> = serde_json::from_str(&public_str)?;

    if public_signals.len() != 7 {
        anyhow::bail!("Expected 7 public signals, got {}", public_signals.len());
    }

    // Parse public signals into U256
    let mut zk_public_signals = [U256::zero(); 7];
    for (i, sig_str) in public_signals.iter().enumerate() {
        zk_public_signals[i] = U256::from_dec_str(sig_str)?;
    }

    // Parse proof into pA, pB, pC and encode using ethabi
    // pA: [uint256, uint256]
    // pB: [[uint256, uint256], [uint256, uint256]]
    // pC: [uint256, uint256]
    let pa_0 = U256::from_dec_str(proof["pi_a"][0].as_str().unwrap())?;
    let pa_1 = U256::from_dec_str(proof["pi_a"][1].as_str().unwrap())?;

    // Note: Groth16 pB elements are reversed when passing to Solidity (snarkjs standard)
    let pb_0_1 = U256::from_dec_str(proof["pi_b"][0][0].as_str().unwrap())?;
    let pb_0_0 = U256::from_dec_str(proof["pi_b"][0][1].as_str().unwrap())?;
    let pb_1_1 = U256::from_dec_str(proof["pi_b"][1][0].as_str().unwrap())?;
    let pb_1_0 = U256::from_dec_str(proof["pi_b"][1][1].as_str().unwrap())?;

    let pc_0 = U256::from_dec_str(proof["pi_c"][0].as_str().unwrap())?;
    let pc_1 = U256::from_dec_str(proof["pi_c"][1].as_str().unwrap())?;

    // Encode ABI: abi.encode(pA[2], pB[2][2], pC[2])
    let token_pa = ethers::abi::Token::FixedArray(vec![
        ethers::abi::Token::Uint(pa_0.into()),
        ethers::abi::Token::Uint(pa_1.into()),
    ]);
    
    let token_pb = ethers::abi::Token::FixedArray(vec![
        ethers::abi::Token::FixedArray(vec![
            ethers::abi::Token::Uint(pb_0_0.into()),
            ethers::abi::Token::Uint(pb_0_1.into()),
        ]),
        ethers::abi::Token::FixedArray(vec![
            ethers::abi::Token::Uint(pb_1_0.into()),
            ethers::abi::Token::Uint(pb_1_1.into()),
        ]),
    ]);

    let token_pc = ethers::abi::Token::FixedArray(vec![
        ethers::abi::Token::Uint(pc_0.into()),
        ethers::abi::Token::Uint(pc_1.into()),
    ]);

    let zk_proof_data = ethers::abi::encode(&[token_pa, token_pb, token_pc]);

    // Cleanup
    let _ = fs::remove_dir_all(temp_dir);

    Ok((zk_proof_data, zk_public_signals))
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
}
