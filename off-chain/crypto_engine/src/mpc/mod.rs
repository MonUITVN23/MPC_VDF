use anyhow::Result;

use crate::dkg::{run_pedersen_dkg, run_pedersen_dkg_default, DkgOutput, PedersenParams};

#[derive(Debug, Clone)]
pub struct CollectiveRandomness {
	pub seed_collective: [u8; 32],
	pub aggregate_signature: Vec<u8>,
	pub aggregated_public_key: Vec<u8>,
}

pub fn init_collective_seed_default() -> Result<CollectiveRandomness> {
	let out = run_pedersen_dkg_default()?;
	Ok(CollectiveRandomness {
		seed_collective: out.seed_collective,
		aggregate_signature: out.aggregate_signature,
		aggregated_public_key: out.aggregated_public_key,
	})
}

pub fn init_collective_seed_with_params(n: usize, t: usize) -> Result<CollectiveRandomness> {
	let out: DkgOutput = run_pedersen_dkg(PedersenParams { n, t })?;
	Ok(CollectiveRandomness {
		seed_collective: out.seed_collective,
		aggregate_signature: out.aggregate_signature,
		aggregated_public_key: out.aggregated_public_key,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn mpc_init_default_returns_seed_and_signature() {
		let result = init_collective_seed_default().unwrap();
		assert_eq!(result.seed_collective.len(), 32);
		assert!(!result.aggregate_signature.is_empty());
		assert!(!result.aggregated_public_key.is_empty());
	}

	#[test]
	fn mpc_init_custom_3_of_4_returns_seed_and_signature() {
		let result = init_collective_seed_with_params(4, 3).unwrap();
		assert_eq!(result.seed_collective.len(), 32);
		assert!(!result.aggregate_signature.is_empty());
	}
}
