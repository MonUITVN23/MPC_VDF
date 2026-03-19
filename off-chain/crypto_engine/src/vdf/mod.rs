use anyhow::{bail, Result};
use classgroup::{
	gmp::mpz::ProbabPrimeResult,
	gmp_classgroup::GmpClassGroup,
	gmp::mpz::Mpz,
	ClassGroup,
};
use sha2::{Digest, Sha256};
use vdf::create_discriminant;

const DEFAULT_DISCRIMINANT_BITS: u16 = 2048;

#[derive(Debug, Clone)]
pub struct WesolowskiVdfOutput {
	pub y_bytes: Vec<u8>,
	pub proof_pi_bytes: Vec<u8>,
}

fn element_len_bytes(int_size_bits: u16) -> usize {
	2 * ((usize::from(int_size_bits) + 16) >> 4)
}

fn serialize_group_element(element: &GmpClassGroup, int_size_bits: u16) -> Result<Vec<u8>> {
	let mut out = vec![0u8; element_len_bytes(int_size_bits)];
	element
		.serialize(&mut out)
		.map_err(|needed| anyhow::anyhow!("serialize buffer too small, required {needed} bytes"))?;
	Ok(out)
}

fn u64_to_be_bytes(v: u64) -> [u8; 8] {
	[
		(v >> 56) as u8,
		(v >> 48) as u8,
		(v >> 40) as u8,
		(v >> 32) as u8,
		(v >> 24) as u8,
		(v >> 16) as u8,
		(v >> 8) as u8,
		v as u8,
	]
}

fn fiat_shamir_hash_to_prime(seed_parts: &[&[u8]]) -> Mpz {
	let mut counter = 0u64;
	loop {
		let mut hasher = Sha256::new();
		hasher.update(b"prime");
		hasher.update(u64_to_be_bytes(counter));
		for part in seed_parts {
			hasher.update(part);
		}

		let digest = hasher.finalize();
		let candidate = Mpz::from(&digest[..16]);
		if candidate.probab_prime(2) != ProbabPrimeResult::NotPrime {
			return candidate;
		}
		counter = counter.wrapping_add(1);
	}
}

pub fn evaluate(seed_collective: &[u8], t: u64, int_size_bits: u16) -> Result<Vec<u8>> {
	if int_size_bits < 256 {
		bail!("int_size_bits must be >= 256 for secure IQCG parameters")
	}

	let discriminant: Mpz = create_discriminant(seed_collective, int_size_bits);
	let mut x = GmpClassGroup::generator_for_discriminant(discriminant);

	for _ in 0..t {
		x.square();
	}

	serialize_group_element(&x, int_size_bits)
}

pub fn generate_proof(
	seed_collective: &[u8],
	y_bytes: &[u8],
	t: u64,
	int_size_bits: u16,
) -> Result<Vec<u8>> {
	if int_size_bits < 256 {
		bail!("int_size_bits must be >= 256 for secure IQCG parameters")
	}

	let discriminant: Mpz = create_discriminant(seed_collective, int_size_bits);
	let x = GmpClassGroup::generator_for_discriminant(discriminant.clone());
	let x_bytes = serialize_group_element(&x, int_size_bits)?;

	let ell = fiat_shamir_hash_to_prime(&[&x_bytes, y_bytes]);

	let mut two_pow_t = Mpz::from(1u64);
	two_pow_t <<= t as usize;

	let q = two_pow_t.clone() / &ell;

	let mut proof_pi = x;
	proof_pi.pow(q);
	serialize_group_element(&proof_pi, int_size_bits)
}

pub fn evaluate_and_generate_proof(seed_collective: &[u8], t: u64) -> Result<WesolowskiVdfOutput> {
	let y_bytes = evaluate(seed_collective, t, DEFAULT_DISCRIMINANT_BITS)?;
	let proof_pi_bytes = generate_proof(seed_collective, &y_bytes, t, DEFAULT_DISCRIMINANT_BITS)?;

	Ok(WesolowskiVdfOutput {
		y_bytes,
		proof_pi_bytes,
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use hex::encode as hex_encode;

	#[test]
	fn evaluate_and_prove_outputs_non_empty_byte_arrays() {
		let seed = b"session_001_seed";
		let t = 128;

		let out = evaluate_and_generate_proof(seed, t).unwrap();
		assert!(!out.y_bytes.is_empty());
		assert!(!out.proof_pi_bytes.is_empty());
	}

	#[test]
	fn proof_is_verifiable_by_vdf_crate() {
		let seed = b"session_001_seed";
		let t = 128;

		let out = evaluate_and_generate_proof(seed, t).unwrap();
		let mut proof_blob = Vec::with_capacity(out.y_bytes.len() + out.proof_pi_bytes.len());
		proof_blob.extend_from_slice(&out.y_bytes);
		proof_blob.extend_from_slice(&out.proof_pi_bytes);

		let wesolowski = vdf::WesolowskiVDFParams(DEFAULT_DISCRIMINANT_BITS).new();
		assert!(wesolowski.verify(seed, t, &proof_blob).is_ok());
	}

	#[test]
	fn export_mock_data_for_solidity_with_t_2_pow_20() {
		let seed = b"session_001_seed";
		let t: u64 = 1 << 20;
		let discriminant_bits: u16 = 1024;

		let y_bytes = evaluate(seed, t, discriminant_bits).unwrap();
		let proof_pi_bytes = generate_proof(seed, &y_bytes, t, discriminant_bits).unwrap();

		println!("pi_length_bytes={}", proof_pi_bytes.len());
		println!("seed_hex={}", hex_encode(seed));
		println!("y_hex={}", hex_encode(&y_bytes));
		println!("pi_hex={}", hex_encode(&proof_pi_bytes));

		let mut proof_blob = Vec::with_capacity(y_bytes.len() + proof_pi_bytes.len());
		proof_blob.extend_from_slice(&y_bytes);
		proof_blob.extend_from_slice(&proof_pi_bytes);

		let wesolowski = vdf::WesolowskiVDFParams(discriminant_bits).new();
		assert!(wesolowski.verify(seed, t, &proof_blob).is_ok());

		assert!(
			(120..=136).contains(&proof_pi_bytes.len()),
			"proof length {} is not in expected ~128-byte range",
			proof_pi_bytes.len()
		);
	}
}
