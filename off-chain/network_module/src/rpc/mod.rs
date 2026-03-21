use std::sync::Arc;

use anyhow::{Context, Result};
use ethers::prelude::*;

abigen!(
	RandomSender,
	r#"[
		event LogRequest(uint256 indexed requestId, uint256 userSeed)
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
	sender_address: Address,
	from_block: u64,
	to_block: u64,
) -> Result<Vec<LogRequestFilter>> {
	let sender = RandomSender::new(sender_address, provider);
	const MAX_BLOCK_RANGE: u64 = 10;

	let mut all_events = Vec::new();
	let mut start = from_block;

	while start <= to_block {
		let end = start
			.saturating_add(MAX_BLOCK_RANGE.saturating_sub(1))
			.min(to_block);

		let mut events = sender
			.event::<LogRequestFilter>()
			.from_block(start)
			.to_block(end)
			.query()
			.await
			.with_context(|| {
				format!(
					"failed querying LogRequest events in block range {start}..={end}"
				)
			})?;

		all_events.append(&mut events);

		if end == u64::MAX {
			break;
		}
		start = end.saturating_add(1);
	}

	Ok(all_events)
}
