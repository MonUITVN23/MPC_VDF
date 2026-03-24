use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use ethers::prelude::*;
use tokio::time::sleep;

abigen!(
	RandomRouter,
	r#"[
		event LogRequest(uint256 indexed requestId, uint256 userSeed, uint256 timestamp)
	]"#
);

pub type EthProvider = Provider<Http>;

pub async fn current_block(provider: &Arc<EthProvider>) -> Result<u64> {
	let bn = provider
		.get_block_number()
		.await
		.context("failed to fetch latest block")?;
	Ok(bn.as_u64())
}

pub async fn fetch_log_requests_in_range(
	provider: Arc<EthProvider>,
	router_address: Address,
	from_block: u64,
	to_block: u64,
) -> Result<Vec<LogRequestFilter>> {
	let router = RandomRouter::new(router_address, provider);
	let max_block_range = std::env::var("RELAYER_MAX_BLOCK_RANGE")
		.ok()
		.and_then(|v| v.parse::<u64>().ok())
		.filter(|v| *v > 0)
		.unwrap_or(5);
	let rpc_max_retries = std::env::var("RELAYER_RPC_MAX_RETRIES")
		.ok()
		.and_then(|v| v.parse::<u32>().ok())
		.filter(|v| *v > 0)
		.unwrap_or(6);
	let rpc_retry_base_ms = std::env::var("RELAYER_RPC_RETRY_BASE_MS")
		.ok()
		.and_then(|v| v.parse::<u64>().ok())
		.filter(|v| *v > 0)
		.unwrap_or(800);

	let mut all_events = Vec::new();
	let mut start = from_block;

	while start <= to_block {
		let end = start
			.saturating_add(max_block_range.saturating_sub(1))
			.min(to_block);

		let mut events = Vec::new();
		let mut last_error: Option<anyhow::Error> = None;

		for attempt in 1..=rpc_max_retries {
			match router
				.event::<LogRequestFilter>()
				.from_block(start)
				.to_block(end)
				.query()
				.await
			{
				Ok(found) => {
					events = found;
					last_error = None;
					break;
				}
				Err(error) => {
					let msg = error.to_string();
					let is_rate_limited = msg.contains("429")
						|| msg.contains("compute units per second")
						|| msg.to_lowercase().contains("rate limit");

					if is_rate_limited && attempt < rpc_max_retries {
						let delay_ms = rpc_retry_base_ms.saturating_mul(2u64.saturating_pow(attempt - 1));
						sleep(Duration::from_millis(delay_ms)).await;
						continue;
					}

					last_error = Some(anyhow::anyhow!(error));
					break;
				}
			}
		}

		if let Some(error) = last_error {
			return Err(error).with_context(|| {
				format!(
					"failed querying LogRequest events in block range {start}..={end}"
				)
			});
		}

		all_events.append(&mut events);

		if end == u64::MAX {
			break;
		}
		start = end.saturating_add(1);
	}

	Ok(all_events)
}
