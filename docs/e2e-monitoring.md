# E2E Monitoring (Temporary)

Artifacts generated:
- `docs/e2e-monitor.json`
- `docs/e2e-monitor.svg`

## Generate
Run from `contracts/`:

```bash
npm run monitor:e2e
```

With wider block lookback:

```bash
E2E_REPORT_LOOKBACK_BLOCKS=2000 E2E_REPORT_MAX_BLOCK_RANGE=10 npm run monitor:e2e
```

## Metrics in dashboard
- Total requests observed on source chain (`RandomSender.LogRequest`)
- Relayed requests observed on destination chain (`RandomReceiver.OptimisticResultSubmitted`)
- Pending requests (source exists, destination event missing)
- Approx latency in seconds (`destinationBlock.timestamp - sourceBlock.timestamp`) when both events exist

## Notes
- Current RPC free tier enforces small `eth_getLogs` ranges. Script chunks ranges via `E2E_REPORT_MAX_BLOCK_RANGE` (default `10`).
- If `Relayed=0`, it means destination submit events were not found on-chain in the scanned window (possibly tx not mined/replaced/reverted).
