use bls_signatures::{PrivateKey, Serialize};
use rand::thread_rng;
use sp1_sdk::{
    blocking::{ProveRequest, Prover, ProverClient},
    include_elf, Elf, SP1Stdin,
};
use std::time::Instant;

const ZK_BLS_ELF: Elf = include_elf!("fibonacci-program");

fn main() {
    sp1_sdk::utils::setup_logger();
    dotenv::dotenv().ok();

    let client = ProverClient::new();
    let pk = client.setup(ZK_BLS_ELF).expect("failed to setup elf");

    println!("Generating mock BLS12-381 keypair and signature...");
    let mut rng = thread_rng();
    let private_key = PrivateKey::generate(&mut rng);
    let public_key = private_key.public_key();
    let message = b"Hello SP1 ZK-SNARK Optimistic Verification!";
    let signature = private_key.sign(message);

    let public_key_bytes = public_key.as_bytes();
    let signature_bytes = signature.as_bytes();
    let message_bytes = message.to_vec();

    let mut stdin = SP1Stdin::new();
    stdin.write(&public_key_bytes);
    stdin.write(&signature_bytes);
    stdin.write(&message_bytes);

    println!("Starting ZK proving process (this may take a few seconds)...");
    let start = Instant::now();
    let proof = client
        .prove(&pk, stdin)
        .run()
        .expect("ZK Proving failed!");
    let proving_secs = start.elapsed().as_secs_f64();

    println!("Proof generated successfully!");
    println!("Proving time: {:.3} seconds", proving_secs);

    let committed_msg = proof.public_values.as_slice();
    println!(
        "Committed Message from ZK: {:?}",
        String::from_utf8_lossy(committed_msg)
    );
    assert_eq!(committed_msg, message, "Committed message mismatch!");

    client
        .verify(&proof, pk.verifying_key(), None)
        .expect("ZK Proof Verification failed!");
    println!("ZK Proof verified successfully Local E2E!");
}
