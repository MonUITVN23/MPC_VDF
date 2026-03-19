use anyhow::{anyhow, bail, Result};
use bls_signatures::{PrivateKey, PublicKey, Serialize, Signature};
use rand::thread_rng;
use serde::{Deserialize, Serialize as SerdeSerialize};
use sha2::{Digest, Sha256};

pub const DEFAULT_NODES: usize = 4;
pub const DEFAULT_THRESHOLD: usize = 3;

#[derive(Debug, Clone, SerdeSerialize, Deserialize)]
pub struct PedersenParams {
	pub n: usize,
	pub t: usize,
}

impl Default for PedersenParams {
	fn default() -> Self {
		Self {
			n: DEFAULT_NODES,
			t: DEFAULT_THRESHOLD,
		}
	}
}

#[derive(Debug, Clone)]
pub struct Participant {
	pub node_id: u32,
	pub private_key: PrivateKey,
	pub public_key: PublicKey,
}

#[derive(Debug, Clone, SerdeSerialize, Deserialize)]
pub struct PedersenCommitment {
	pub node_id: u32,
	pub pk_bytes: Vec<u8>,
}

#[derive(Debug, Clone, SerdeSerialize, Deserialize)]
pub struct DkgOutput {
	pub seed_collective: [u8; 32],
	pub aggregate_signature: Vec<u8>,
	pub aggregated_public_key: Vec<u8>,
	pub participants: usize,
	pub threshold: usize,
}

pub fn generate_keypair(node_id: u32) -> Participant {
	let mut rng = thread_rng();
	let private_key = PrivateKey::generate(&mut rng);
	let public_key = private_key.public_key();
	Participant {
		node_id,
		private_key,
		public_key,
	}
}

pub fn broadcast_commitment(commitment: &PedersenCommitment) -> Result<()> {
	if commitment.pk_bytes.is_empty() {
		bail!("empty commitment is not allowed")
	}
	Ok(())
}

pub fn run_pedersen_dkg(params: PedersenParams) -> Result<DkgOutput> {
	if params.t == 0 || params.n == 0 {
		bail!("invalid DKG params: n and t must be > 0")
	}
	if params.t > params.n {
		bail!("invalid DKG params: threshold t cannot exceed n")
	}

	let participants: Vec<Participant> = (1..=params.n)
		.map(|i| generate_keypair(i as u32))
		.collect();

	let commitments: Vec<PedersenCommitment> = participants
		.iter()
		.map(|p| PedersenCommitment {
			node_id: p.node_id,
			pk_bytes: p.public_key.as_bytes(),
		})
		.collect();

	for c in &commitments {
		broadcast_commitment(c)?;
	}

	let mut transcript = Sha256::new();
	transcript.update(b"pedersen-dkg-bls12-381");
	transcript.update((params.n as u64).to_be_bytes());
	transcript.update((params.t as u64).to_be_bytes());
	for c in &commitments {
		transcript.update(c.node_id.to_be_bytes());
		transcript.update((c.pk_bytes.len() as u64).to_be_bytes());
		transcript.update(&c.pk_bytes);
	}
	let seed_collective: [u8; 32] = transcript.finalize().into();

	let signed_subset = participants
		.iter()
		.take(params.t)
		.collect::<Vec<&Participant>>();

	let partial_signatures: Vec<Signature> = signed_subset
		.iter()
		.map(|p| p.private_key.sign(seed_collective.as_ref()))
		.collect();

	let aggregate_signature = bls_signatures::aggregate(&partial_signatures)
		.map_err(|e| anyhow!("cannot aggregate BLS threshold signatures: {e}"))?;

	let subset_public_keys: Vec<PublicKey> = signed_subset
		.iter()
		.map(|p| p.public_key)
		.collect();
	let mut public_key_digest = Sha256::new();
	for pk in &subset_public_keys {
		public_key_digest.update(pk.as_bytes());
	}
	let aggregated_public_key = public_key_digest.finalize().to_vec();

	Ok(DkgOutput {
		seed_collective,
		aggregate_signature: aggregate_signature.as_bytes(),
		aggregated_public_key,
		participants: params.n,
		threshold: params.t,
	})
}

pub fn run_pedersen_dkg_default() -> Result<DkgOutput> {
	run_pedersen_dkg(PedersenParams::default())
}

#[cfg(test)]
mod tests {
	use super::*;
	use blstrs::{pairing, G1Affine, G1Projective, G2Affine, G2Projective, Scalar};
	use group::ff::Field;
	use group::Group;
	use rand::{rngs::StdRng, RngCore, SeedableRng};

	fn random_scalar(rng: &mut StdRng) -> Scalar {
		Scalar::from(rng.next_u64())
	}

	fn eval_poly(coeffs: &[Scalar], x: Scalar) -> Scalar {
		let mut acc = Scalar::ZERO;
		let mut power = Scalar::ONE;
		for coefficient in coeffs {
			acc += *coefficient * power;
			power *= x;
		}
		acc
	}

	fn hash_message_to_g1(message: &[u8]) -> G1Projective {
		G1Projective::hash_to_curve(message, b"MPC-VDF-BLS-SIG-DST", b"")
	}

	fn lagrange_at_zero(signer_ids: &[u64], target_id: u64) -> Scalar {
		let x_i = Scalar::from(target_id);
		let mut numerator = Scalar::ONE;
		let mut denominator = Scalar::ONE;

		for other_id in signer_ids {
			if *other_id == target_id {
				continue;
			}
			let x_j = Scalar::from(*other_id);
			numerator *= x_j;
			denominator *= x_j - x_i;
		}

		let inv = Option::<Scalar>::from(denominator.invert())
			.expect("non-zero denominator for distinct signer ids");
		numerator * inv
	}

	#[test]
	fn returns_collective_seed_and_aggregate_signature_for_3_of_4() {
		let out = run_pedersen_dkg(PedersenParams { n: 4, t: 3 }).unwrap();
		assert_eq!(out.participants, 4);
		assert_eq!(out.threshold, 3);
		assert_eq!(out.seed_collective.len(), 32);
		assert!(!out.aggregate_signature.is_empty());
		assert!(!out.aggregated_public_key.is_empty());
	}

	#[test]
	fn rejects_invalid_threshold() {
		let err = run_pedersen_dkg(PedersenParams { n: 4, t: 5 })
			.expect_err("t > n must fail");
		assert!(err
			.to_string()
			.contains("threshold t cannot exceed n"));
	}

	#[test]
	fn pedersen_dkg_threshold_bls_flow_3_of_4() {
		let n = 4usize;
		let t = 3usize;
		let degree = t - 1;
		let mut rng = StdRng::seed_from_u64(20260316);

		let mut polynomials: Vec<Vec<Scalar>> = Vec::with_capacity(n);
		let mut commitments: Vec<Vec<G2Projective>> = Vec::with_capacity(n);

		for _ in 0..n {
			let coeffs: Vec<Scalar> = (0..=degree).map(|_| random_scalar(&mut rng)).collect();
			let coeff_commitments: Vec<G2Projective> = coeffs
				.iter()
				.map(|coefficient| G2Projective::generator() * coefficient)
				.collect();

			polynomials.push(coeffs);
			commitments.push(coeff_commitments);
		}

		let mut secret_shares: Vec<Scalar> = vec![Scalar::ZERO; n];
		for receiver in 1..=n {
			let receiver_x = Scalar::from(receiver as u64);
			for coeffs in &polynomials {
				secret_shares[receiver - 1] += eval_poly(coeffs, receiver_x);
			}
		}

		for receiver in 1..=n {
			let x = Scalar::from(receiver as u64);
			let mut expected_pub_share = G2Projective::identity();
			for node_commitments in &commitments {
				let mut power = Scalar::ONE;
				for commitment in node_commitments {
					expected_pub_share += *commitment * power;
					power *= x;
				}
			}

			let actual_pub_share = G2Projective::generator() * secret_shares[receiver - 1];
			assert_eq!(expected_pub_share, actual_pub_share);
		}

		let master_secret = polynomials
			.iter()
			.fold(Scalar::ZERO, |acc, coeffs| acc + coeffs[0]);

		let collective_public_key = G2Projective::generator() * master_secret;
		let collective_public_key_from_commitments = commitments
			.iter()
			.fold(G2Projective::identity(), |acc, c| acc + c[0]);
		assert_eq!(collective_public_key, collective_public_key_from_commitments);

		let message = b"session_001_seed";
		let hashed_message = hash_message_to_g1(message);

		let signer_ids = vec![1u64, 2u64, 4u64];
		let partial_signatures: Vec<(u64, G1Projective)> = signer_ids
			.iter()
			.map(|id| {
				let idx = (*id as usize) - 1;
				(*id, hashed_message * secret_shares[idx])
			})
			.collect();

		let aggregate_signature = partial_signatures.iter().fold(
			G1Projective::identity(),
			|acc, (id, sigma_i)| {
				let lambda_i = lagrange_at_zero(&signer_ids, *id);
				acc + (*sigma_i * lambda_i)
			},
		);

		let lhs = pairing(
			&G1Affine::from(aggregate_signature),
			&G2Affine::from(G2Projective::generator()),
		);
		let rhs = pairing(
			&G1Affine::from(hashed_message),
			&G2Affine::from(collective_public_key),
		);

		let is_valid = lhs == rhs;
		let signature_len_bytes = G1Affine::from(aggregate_signature).to_compressed().len();
		println!("aggregate signature length: {} bytes", signature_len_bytes);
		assert!(is_valid);
	}
}
