pub mod vdf;
pub mod mpc;
pub mod dkg;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Instant;

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineOutput {
	pub metadata: PipelineMetadata,
	pub payload: RandomnessExport,
}

pub fn run_randomness_pipeline_with_seed(
	session_id: &str,
	seed_user: &[u8],
	t: u64,
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

	Ok(PipelineOutput {
		metadata: PipelineMetadata {
			session_id: session_id.to_owned(),
			seed_collective,
			benchmark: BenchmarkMetrics { t2_mpc_ms, t3_vdf_ms },
		},
		payload: RandomnessExport {
			y: vdf_output.y_bytes,
			pi: vdf_output.proof_pi_bytes,
			aggregate_signature: collective.aggregate_signature,
		},
	})
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
