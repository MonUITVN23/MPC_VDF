use anyhow::{Context, Result};
use crypto_engine::run_randomness_pipeline_with_seed;
use serde::Serialize;

#[derive(Serialize)]
struct Output {
    t_value: u64,
    prover_time_ms: u128,
    seed_collective_hex: String,
    pi_hex: String,
}

fn parse_arg(args: &[String], name: &str) -> Option<String> {
    let prefix = format!("--{name}=");
    for arg in args {
        if let Some(value) = arg.strip_prefix(&prefix) {
            return Some(value.to_owned());
        }
    }

    for idx in 0..args.len() {
        if args[idx] == format!("--{name}") && idx + 1 < args.len() {
            return Some(args[idx + 1].clone());
        }
    }
    None
}

fn parse_seed_hex(seed_hex: Option<String>) -> Result<Vec<u8>> {
    let fallback = "686172646861742d6c6f63616c2d62656e63682d73656564".to_owned();
    let value = seed_hex.unwrap_or(fallback);
    let trimmed = value.strip_prefix("0x").unwrap_or(&value);
    let decoded = hex::decode(trimmed).context("invalid --seed-hex")?;
    Ok(decoded)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let t_value = parse_arg(&args, "t")
        .or_else(|| parse_arg(&args, "t-value"))
        .unwrap_or_else(|| "32768".to_owned())
        .parse::<u64>()
        .context("invalid --t")?;

    let session_id = parse_arg(&args, "session-id").unwrap_or_else(|| {
        format!(
            "hardhat-local-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|v| v.as_millis())
                .unwrap_or_default()
        )
    });

    let seed = parse_seed_hex(parse_arg(&args, "seed-hex"))?;

    let out = run_randomness_pipeline_with_seed(&session_id, &seed, t_value)
        .with_context(|| format!("pipeline failed for t={t_value}"))?;

    let payload = Output {
        t_value,
        prover_time_ms: out.metadata.benchmark.t3_vdf_ms,
        seed_collective_hex: format!("0x{}", hex::encode(out.metadata.seed_collective)),
        pi_hex: format!("0x{}", hex::encode(out.payload.pi)),
    };

    println!("{}", serde_json::to_string(&payload)?);
    Ok(())
}
