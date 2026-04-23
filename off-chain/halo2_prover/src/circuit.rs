




use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    pasta::Fp,
    plonk::{
        Advice, Circuit, Column, ConstraintSystem, Error, Expression, Instance, Selector,
    },
    poly::Rotation,
};
use sha2::{Digest, Sha256};


pub const PK_BYTES: usize = 48;
pub const SIG_BYTES: usize = 96;
pub const MSG_BYTES: usize = 32;
pub const TOTAL_INPUT_BYTES: usize = PK_BYTES + SIG_BYTES + MSG_BYTES; 

#[derive(Debug, Clone)]
pub struct BlsCommitmentConfig {
    advice: [Column<Advice>; 3],
    instance: Column<Instance>,
    s_byte_range: Selector,
    s_sha256_check: Selector,
}


#[derive(Debug, Clone)]
pub struct BlsCommitmentInput {
    pub pk: Vec<u8>,
    pub sig: Vec<u8>,
    pub msg: Vec<u8>,
    pub commitment_hi: u128,
    pub commitment_lo: u128,
    pub pk_hash_hi: u128,
    pub pk_hash_lo: u128,
    pub payload_hash_hi: u128,
    pub payload_hash_lo: u128,
    pub request_id: u64,
}

impl BlsCommitmentInput {
    pub fn from_raw(
        pk: &[u8], sig: &[u8], msg: &[u8],
        y: &[u8], pi: &[u8], modulus: &[u8],
        request_id: u64,
    ) -> Self {
        let pk_padded = pad_or_truncate(pk, PK_BYTES);
        let sig_padded = pad_or_truncate(sig, SIG_BYTES);
        let msg_padded = pad_or_truncate(msg, MSG_BYTES);

        let mut hasher = Sha256::new();
        hasher.update(&pk_padded);
        hasher.update(&sig_padded);
        hasher.update(&msg_padded);
        let commitment_hash: [u8; 32] = hasher.finalize().into();
        let (commitment_hi, commitment_lo) = split_256_to_128(&commitment_hash);

        let mut hasher = Sha256::new();
        hasher.update(&pk_padded);
        let pk_hash: [u8; 32] = hasher.finalize().into();
        let (pk_hash_hi, pk_hash_lo) = split_256_to_128(&pk_hash);

        let mut request_id_buf = [0u8; 32];
        request_id_buf[24..32].copy_from_slice(&request_id.to_be_bytes());
        let mut hasher = Sha256::new();
        hasher.update(&request_id_buf);
        hasher.update(y);
        hasher.update(pi);
        hasher.update(&msg_padded);
        hasher.update(modulus);
        let payload_hash: [u8; 32] = hasher.finalize().into();
        let (payload_hash_hi, payload_hash_lo) = split_256_to_128(&payload_hash);

        Self {
            pk: pk_padded, sig: sig_padded, msg: msg_padded,
            commitment_hi, commitment_lo,
            pk_hash_hi, pk_hash_lo,
            payload_hash_hi, payload_hash_lo,
            request_id,
        }
    }
}


fn fp_from_u128(val: u128) -> Fp {
    let lo = val as u64;
    let hi = (val >> 64) as u64;
    Fp::from(lo) + Fp::from(hi) * Fp::from(1u64 << 32) * Fp::from(1u64 << 32)
}


#[derive(Debug, Clone, Default)]
pub struct BlsCommitmentCircuit {
    pub input: Option<BlsCommitmentInput>,
}

impl Circuit<Fp> for BlsCommitmentCircuit {
    type Config = BlsCommitmentConfig;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        let advice = [
            meta.advice_column(),
            meta.advice_column(),
            meta.advice_column(),
        ];
        let instance = meta.instance_column();
        let s_byte_range = meta.selector();
        let s_sha256_check = meta.selector();

        meta.enable_equality(instance);
        for col in &advice {
            meta.enable_equality(*col);
        }

        
        meta.create_gate("byte_range_check", |meta| {
            let s = meta.query_selector(s_byte_range);
            let value = meta.query_advice(advice[0], Rotation::cur());
            let high = meta.query_advice(advice[1], Rotation::cur());
            let low = meta.query_advice(advice[2], Rotation::cur());
            vec![s * (high * Expression::Constant(Fp::from(16)) + low - value)]
        });

        
        meta.create_gate("sha256_check", |meta| {
            let s = meta.query_selector(s_sha256_check);
            let computed = meta.query_advice(advice[0], Rotation::cur());
            let expected = meta.query_advice(advice[1], Rotation::cur());
            vec![s * (computed - expected)]
        });

        BlsCommitmentConfig { advice, instance, s_byte_range, s_sha256_check }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let input = &self.input;

        
        let (chi, clo, phi, plo) = layouter.assign_region(
            || "private inputs and hashes",
            |mut region| {
                let mut row = 0;

                
                for i in 0..TOTAL_INPUT_BYTES {
                    let byte_val: Value<Fp> = input.as_ref()
                        .map(|inp| {
                            let b = if i < PK_BYTES { inp.pk[i] }
                                    else if i < PK_BYTES + SIG_BYTES { inp.sig[i - PK_BYTES] }
                                    else { inp.msg[i - PK_BYTES - SIG_BYTES] };
                            Fp::from(b as u64)
                        })
                        .map_or(Value::unknown(), Value::known);

                    let high_val: Value<Fp> = input.as_ref()
                        .map(|inp| {
                            let b = if i < PK_BYTES { inp.pk[i] }
                                    else if i < PK_BYTES + SIG_BYTES { inp.sig[i - PK_BYTES] }
                                    else { inp.msg[i - PK_BYTES - SIG_BYTES] };
                            Fp::from((b >> 4) as u64)
                        })
                        .map_or(Value::unknown(), Value::known);

                    let low_val: Value<Fp> = input.as_ref()
                        .map(|inp| {
                            let b = if i < PK_BYTES { inp.pk[i] }
                                    else if i < PK_BYTES + SIG_BYTES { inp.sig[i - PK_BYTES] }
                                    else { inp.msg[i - PK_BYTES - SIG_BYTES] };
                            Fp::from((b & 0x0F) as u64)
                        })
                        .map_or(Value::unknown(), Value::known);

                    region.assign_advice(|| format!("byte_{i}"), config.advice[0], row, || byte_val)?;
                    region.assign_advice(|| format!("high_{i}"), config.advice[1], row, || high_val)?;
                    region.assign_advice(|| format!("low_{i}"), config.advice[2], row, || low_val)?;
                    config.s_byte_range.enable(&mut region, row)?;
                    row += 1;
                }

                
                let c_hash = input.as_ref().map(|inp| {
                    let mut h = Sha256::new();
                    h.update(&inp.pk); h.update(&inp.sig); h.update(&inp.msg);
                    let r: [u8; 32] = h.finalize().into(); r
                });
                let (c_hi_v, c_lo_v) = match c_hash {
                    Some(h) => { let (hi, lo) = split_256_to_128(&h); (Value::known(fp_from_u128(hi)), Value::known(fp_from_u128(lo))) }
                    None => (Value::unknown(), Value::unknown()),
                };

                let chi = region.assign_advice(|| "c_hi", config.advice[0], row, || c_hi_v)?;
                region.assign_advice(|| "exp_c_hi", config.advice[1], row,
                    || input.as_ref().map(|i| fp_from_u128(i.commitment_hi)).map_or(Value::unknown(), Value::known))?;
                config.s_sha256_check.enable(&mut region, row)?;
                row += 1;

                let clo = region.assign_advice(|| "c_lo", config.advice[0], row, || c_lo_v)?;
                region.assign_advice(|| "exp_c_lo", config.advice[1], row,
                    || input.as_ref().map(|i| fp_from_u128(i.commitment_lo)).map_or(Value::unknown(), Value::known))?;
                config.s_sha256_check.enable(&mut region, row)?;
                row += 1;

                
                let pk_h = input.as_ref().map(|inp| {
                    let mut h = Sha256::new();
                    h.update(&inp.pk);
                    let r: [u8; 32] = h.finalize().into(); r
                });
                let (ph_hi_v, ph_lo_v) = match pk_h {
                    Some(h) => { let (hi, lo) = split_256_to_128(&h); (Value::known(fp_from_u128(hi)), Value::known(fp_from_u128(lo))) }
                    None => (Value::unknown(), Value::unknown()),
                };

                let phi = region.assign_advice(|| "ph_hi", config.advice[0], row, || ph_hi_v)?;
                region.assign_advice(|| "exp_ph_hi", config.advice[1], row,
                    || input.as_ref().map(|i| fp_from_u128(i.pk_hash_hi)).map_or(Value::unknown(), Value::known))?;
                config.s_sha256_check.enable(&mut region, row)?;
                row += 1;

                let plo = region.assign_advice(|| "ph_lo", config.advice[0], row, || ph_lo_v)?;
                region.assign_advice(|| "exp_ph_lo", config.advice[1], row,
                    || input.as_ref().map(|i| fp_from_u128(i.pk_hash_lo)).map_or(Value::unknown(), Value::known))?;
                config.s_sha256_check.enable(&mut region, row)?;

                Ok((chi, clo, phi, plo))
            },
        )?;

        
        let (pay_hi, pay_lo, rid) = layouter.assign_region(
            || "public signals",
            |mut region| {
                let pay_hi = region.assign_advice(|| "payload_hi", config.advice[0], 0,
                    || input.as_ref().map(|i| fp_from_u128(i.payload_hash_hi)).map_or(Value::unknown(), Value::known))?;
                let pay_lo = region.assign_advice(|| "payload_lo", config.advice[0], 1,
                    || input.as_ref().map(|i| fp_from_u128(i.payload_hash_lo)).map_or(Value::unknown(), Value::known))?;
                let rid = region.assign_advice(|| "request_id", config.advice[0], 2,
                    || input.as_ref().map(|i| Fp::from(i.request_id)).map_or(Value::unknown(), Value::known))?;
                Ok((pay_hi, pay_lo, rid))
            },
        )?;

        
        layouter.constrain_instance(chi.cell(), config.instance, 0)?;
        layouter.constrain_instance(clo.cell(), config.instance, 1)?;
        layouter.constrain_instance(phi.cell(), config.instance, 2)?;
        layouter.constrain_instance(plo.cell(), config.instance, 3)?;
        layouter.constrain_instance(pay_hi.cell(), config.instance, 4)?;
        layouter.constrain_instance(pay_lo.cell(), config.instance, 5)?;
        layouter.constrain_instance(rid.cell(), config.instance, 6)?;

        Ok(())
    }
}


pub fn split_256_to_128(hash: &[u8; 32]) -> (u128, u128) {
    let hi = u128::from_be_bytes(hash[0..16].try_into().unwrap());
    let lo = u128::from_be_bytes(hash[16..32].try_into().unwrap());
    (hi, lo)
}


pub fn pad_or_truncate(data: &[u8], target_len: usize) -> Vec<u8> {
    let mut result = vec![0u8; target_len];
    let n = data.len().min(target_len);
    result[..n].copy_from_slice(&data[..n]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_proofs::dev::MockProver;

    #[test]
    fn test_valid_circuit() {
        let input = BlsCommitmentInput::from_raw(
            &vec![1u8; 48], &vec![2u8; 96], &vec![3u8; 32],
            &vec![4u8; 32], &vec![5u8; 32], &vec![6u8; 32], 42,
        );
        let circuit = BlsCommitmentCircuit { input: Some(input.clone()) };
        let pi = vec![
            fp_from_u128(input.commitment_hi), fp_from_u128(input.commitment_lo),
            fp_from_u128(input.pk_hash_hi), fp_from_u128(input.pk_hash_lo),
            fp_from_u128(input.payload_hash_hi), fp_from_u128(input.payload_hash_lo),
            Fp::from(input.request_id),
        ];
        let prover = MockProver::run(10, &circuit, vec![pi]).unwrap();
        prover.assert_satisfied();
    }

    #[test]
    fn test_wrong_commitment_rejected() {
        let input = BlsCommitmentInput::from_raw(
            &vec![1u8; 48], &vec![2u8; 96], &vec![3u8; 32],
            &vec![4u8; 32], &vec![5u8; 32], &vec![6u8; 32], 42,
        );
        let circuit = BlsCommitmentCircuit { input: Some(input.clone()) };
        let pi = vec![
            fp_from_u128(input.commitment_hi + 1), 
            fp_from_u128(input.commitment_lo),
            fp_from_u128(input.pk_hash_hi), fp_from_u128(input.pk_hash_lo),
            fp_from_u128(input.payload_hash_hi), fp_from_u128(input.payload_hash_lo),
            Fp::from(input.request_id),
        ];
        let prover = MockProver::run(10, &circuit, vec![pi]).unwrap();
        assert!(prover.verify().is_err());
    }
}
