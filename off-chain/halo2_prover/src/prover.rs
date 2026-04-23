



use anyhow::{Context, Result};
use halo2_proofs::{
    pasta::{EqAffine, Fp},
    plonk::{create_proof, keygen_pk, keygen_vk, verify_proof, ProvingKey, VerifyingKey, SingleVerifier},
    poly::commitment::Params,
    transcript::{Blake2bRead, Blake2bWrite},
};
use rand::rngs::OsRng;
use std::time::Instant;

use crate::circuit::{BlsCommitmentCircuit, BlsCommitmentInput};


pub const K: u32 = 19;


#[derive(Debug, Clone)]
pub struct Halo2ProofResult {
    pub proof_bytes: Vec<u8>,
    pub public_signals: Vec<String>,
    pub proving_time_ms: u128,
}


pub fn generate_params(k: u32) -> Params<EqAffine> {
    Params::<EqAffine>::new(k)
}


pub fn generate_keys(
    params: &Params<EqAffine>,
) -> Result<(ProvingKey<EqAffine>, VerifyingKey<EqAffine>)> {
    let empty = BlsCommitmentCircuit::default();
    let vk = keygen_vk(params, &empty).context("keygen_vk failed")?;
    let pk = keygen_pk(params, vk.clone(), &empty).context("keygen_pk failed")?;
    Ok((pk, vk))
}


fn fp_from_u128(val: u128) -> Fp {
    let lo = val as u64;
    let hi = (val >> 64) as u64;
    Fp::from(lo) + Fp::from(hi) * Fp::from(1u64 << 32) * Fp::from(1u64 << 32)
}


pub fn prove(
    params: &Params<EqAffine>,
    pk: &ProvingKey<EqAffine>,
    input: &BlsCommitmentInput,
) -> Result<Halo2ProofResult> {
    let start = Instant::now();

    let circuit = BlsCommitmentCircuit { input: Some(input.clone()) };

    let public_inputs = vec![
        fp_from_u128(input.commitment_hi),
        fp_from_u128(input.commitment_lo),
        fp_from_u128(input.pk_hash_hi),
        fp_from_u128(input.pk_hash_lo),
        fp_from_u128(input.payload_hash_hi),
        fp_from_u128(input.payload_hash_lo),
        Fp::from(input.request_id),
    ];

    let mut transcript = Blake2bWrite::<_, _, halo2_proofs::transcript::Challenge255<_>>::init(vec![]);

    create_proof(
        params,
        pk,
        &[circuit],
        &[&[&public_inputs]],
        OsRng,
        &mut transcript,
    )
    .context("create_proof failed")?;

    let proof_bytes = transcript.finalize();
    let proving_time_ms = start.elapsed().as_millis();

    let public_signals: Vec<String> = public_inputs
        .iter()
        .map(|f| format_field_element(f))
        .collect();

    Ok(Halo2ProofResult { proof_bytes, public_signals, proving_time_ms })
}


pub fn verify(
    params: &Params<EqAffine>,
    vk: &VerifyingKey<EqAffine>,
    proof_bytes: &[u8],
    public_inputs: &[Fp],
) -> Result<bool> {
    let _msm = params.empty_msm();
    let mut transcript = Blake2bRead::<_, _, halo2_proofs::transcript::Challenge255<_>>::init(proof_bytes);
    let strategy = SingleVerifier::new(params);

    let result = verify_proof(
        params,
        vk,
        strategy,
        &[&[public_inputs]],
        &mut transcript,
    );

    Ok(result.is_ok())
}


pub fn build_public_inputs(input: &BlsCommitmentInput) -> Vec<Fp> {
    vec![
        fp_from_u128(input.commitment_hi),
        fp_from_u128(input.commitment_lo),
        fp_from_u128(input.pk_hash_hi),
        fp_from_u128(input.pk_hash_lo),
        fp_from_u128(input.payload_hash_hi),
        fp_from_u128(input.payload_hash_lo),
        Fp::from(input.request_id),
    ]
}


pub fn serialize_params(params: &Params<EqAffine>) -> Vec<u8> {
    let mut buf = Vec::new();
    params.write(&mut buf).expect("failed to serialize params");
    buf
}


pub fn deserialize_params(data: &[u8]) -> Result<Params<EqAffine>> {
    Params::<EqAffine>::read(&mut &data[..]).context("failed to deserialize params")
}


fn format_field_element(f: &Fp) -> String {
    use ff::PrimeField;
    let bytes = f.to_repr();
    let mut value = ethers::types::U256::zero();
    for (i, &byte) in bytes.iter().enumerate() {
        value = value + ethers::types::U256::from(byte)
            * ethers::types::U256::from(2u64).pow(ethers::types::U256::from(i * 8));
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_input() -> BlsCommitmentInput {
        BlsCommitmentInput::from_raw(
            &vec![1u8; 48], &vec![2u8; 96], &vec![3u8; 32],
            &vec![4u8; 32], &vec![5u8; 32], &vec![6u8; 32], 42,
        )
    }

    #[test]
    fn test_prove_verify() {
        let params = generate_params(K);
        let (pk, vk) = generate_keys(&params).unwrap();
        let input = sample_input();
        let result = prove(&params, &pk, &input).unwrap();
        assert!(!result.proof_bytes.is_empty());
        assert_eq!(result.public_signals.len(), 7);

        let pi = build_public_inputs(&input);
        assert!(verify(&params, &vk, &result.proof_bytes, &pi).unwrap());
    }

    #[test]
    fn test_tampered_inputs_fail() {
        let params = generate_params(K);
        let (pk, vk) = generate_keys(&params).unwrap();
        let input = sample_input();
        let result = prove(&params, &pk, &input).unwrap();

        let mut bad = build_public_inputs(&input);
        bad[0] = Fp::from(999999u64);
        assert!(!verify(&params, &vk, &result.proof_bytes, &bad).unwrap());
    }

    #[test]
    fn test_corrupted_proof_fails() {
        let params = generate_params(K);
        let (pk, vk) = generate_keys(&params).unwrap();
        let input = sample_input();
        let result = prove(&params, &pk, &input).unwrap();

        let mut bad = result.proof_bytes.clone();
        if let Some(b) = bad.get_mut(10) { *b ^= 0xFF; }
        let pi = build_public_inputs(&input);
        assert!(!verify(&params, &vk, &bad, &pi).unwrap());
    }

    #[test]
    fn test_different_inputs() {
        let params = generate_params(K);
        let (pk, _) = generate_keys(&params).unwrap();

        let r1 = prove(&params, &pk, &sample_input()).unwrap();
        let i2 = BlsCommitmentInput::from_raw(
            &vec![7u8; 48], &vec![8u8; 96], &vec![9u8; 32],
            &vec![10u8; 32], &vec![11u8; 32], &vec![12u8; 32], 99,
        );
        let r2 = prove(&params, &pk, &i2).unwrap();
        assert_ne!(r1.public_signals, r2.public_signals);
    }
}
