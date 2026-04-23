use std::env;
use std::time::Instant;
use crypto_engine::{mpc, vdf};

fn main() {
    let args: Vec<String> = env::args().collect();
    let t: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(65536);
    let mode = args.get(2).map(|s| s.as_str()).unwrap_or("vdf");

    let collective = mpc::init_collective_seed_default().expect("MPC failed");
    let seed: [u8; 32] = {
        use sha2::{Sha256, Digest};
        let mut h = Sha256::new();
        h.update(b"bench-session");
        h.update(&collective.seed_collective);
        h.finalize().into()
    };

    match mode {
        "vdf" => {
            let start = Instant::now();
            let _out = vdf::evaluate_and_generate_proof(&seed, t).expect("VDF failed");
            let elapsed = start.elapsed().as_millis();
            println!("{}", elapsed);
        }
        "zk" => {
            let elapsed = crypto_engine::bench_zk_only(10);
            println!("{}", elapsed);
        }
        _ => eprintln!("Unknown mode: {}", mode),
    }
}
