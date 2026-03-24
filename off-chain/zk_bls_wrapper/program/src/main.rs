#![no_main]
sp1_zkvm::entrypoint!(main);

use bls_signatures::{PublicKey, Signature, Serialize};

pub fn main() {
    let pk_bytes = sp1_zkvm::io::read::<Vec<u8>>();
    let sig_bytes = sp1_zkvm::io::read::<Vec<u8>>();
    let message_bytes = sp1_zkvm::io::read::<Vec<u8>>();

    let pk = PublicKey::from_bytes(&pk_bytes).expect("ZK_GUEST: Invalid Public Key bytes");
    let sig = Signature::from_bytes(&sig_bytes).expect("ZK_GUEST: Invalid Signature bytes");

    let is_valid = pk.verify(sig, &message_bytes);

    assert!(is_valid, "ZK_GUEST: BLS12-381 Signature verification failed!");

    sp1_zkvm::io::commit_slice(&message_bytes);
}
