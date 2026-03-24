import * as fs from "fs";
import * as path from "path";
import * as dotenv from "dotenv";
import { ethers } from "ethers";

const envCandidates = [
	path.resolve(__dirname, "../../.env"),
	path.resolve(__dirname, "../../../.env"),
];
const envPath = envCandidates.find((p) => fs.existsSync(p));
dotenv.config(envPath ? { path: envPath } : undefined);

type E2ERow = {
	requestId: number;
	sourceTxHash?: string;
	sourceBlock?: number;
	sourceTimestamp?: number;
	relayTxHash?: string;
	relayBlock?: number;
	relayTimestamp?: number;
	latencySeconds?: number;
	status: "relayed" | "pending";
};

const ROUTER_ABI = [
	"event LogRequest(uint256 indexed requestId, uint256 userSeed, uint256 timestamp)",
];

const RECEIVER_ABI = [
	"event OptimisticResultSubmitted(uint256 indexed requestId, address indexed submitter, uint256 challengeDeadline)",
];

function requiredEnv(name: string): string {
	const value = process.env[name];
	if (!value) {
		throw new Error(`Missing required env var: ${name}`);
	}
	return value;
}

function shortHash(hash?: string): string {
	if (!hash) {
		return "-";
	}
	return `${hash.slice(0, 10)}…${hash.slice(-6)}`;
}

function timestampToIso(ts?: number): string {
	if (!ts) {
		return "-";
	}
	return new Date(ts * 1000).toISOString();
}

function escapeXml(input: string): string {
	return input
		.replace(/&/g, "&amp;")
		.replace(/</g, "&lt;")
		.replace(/>/g, "&gt;")
		.replace(/\"/g, "&quot;")
		.replace(/'/g, "&apos;");
}

async function queryEventsChunked(
	contract: ethers.Contract,
	filter: any,
	fromBlock: number,
	toBlock: number,
	maxRange: number,
): Promise<any[]> {
	const all: any[] = [];
	let start = fromBlock;

	while (start <= toBlock) {
		const end = Math.min(start + maxRange - 1, toBlock);
		const logs = (await (contract as any).queryFilter(filter, start, end)) as any[];
		all.push(...logs);
		start = end + 1;
	}

	return all;
}

function toNum(value: bigint | number): number {
	return typeof value === "bigint" ? Number(value) : value;
}

function percentile(sortedValues: number[], p: number): number {
	if (sortedValues.length === 0) {
		return 0;
	}
	if (sortedValues.length === 1) {
		return sortedValues[0];
	}
	const rank = (p / 100) * (sortedValues.length - 1);
	const low = Math.floor(rank);
	const high = Math.ceil(rank);
	if (low === high) {
		return sortedValues[low];
	}
	const weight = rank - low;
	return sortedValues[low] * (1 - weight) + sortedValues[high] * weight;
}

function buildSvg(rows: E2ERow[]): string {
	const relayed = rows.filter((r) => r.status === "relayed");
	const pending = rows.filter((r) => r.status === "pending");
	const latencyValues = relayed.map((r) => r.latencySeconds || 0);
	const maxLatency = Math.max(1, ...latencyValues);
	const avgLatency = relayed.length
		? (latencyValues.reduce((acc, v) => acc + v, 0) / relayed.length).toFixed(2)
		: "0.00";

	const width = 1280;
	const barChartTop = 190;
	const barAreaHeight = 300;
	const barWidth = 70;
	const barGap = 30;
	const tableTop = 560;
	const visibleRows = rows.slice(-8).reverse();
	const height = tableTop + 60 + visibleRows.length * 42 + 30;

	const bars = relayed
		.slice(-10)
		.map((row, index) => {
			const latency = row.latencySeconds || 0;
			const h = Math.max(4, (latency / maxLatency) * (barAreaHeight - 30));
			const x = 80 + index * (barWidth + barGap);
			const y = barChartTop + barAreaHeight - h;

			return `
			<rect x="${x}" y="${y}" width="${barWidth}" height="${h}" rx="8" fill="#4f8cff" />
			<text x="${x + barWidth / 2}" y="${y - 8}" text-anchor="middle" font-size="13" fill="#1f2937">${latency}s</text>
			<text x="${x + barWidth / 2}" y="${barChartTop + barAreaHeight + 20}" text-anchor="middle" font-size="12" fill="#4b5563">#${row.requestId}</text>
			`;
		})
		.join("\n");

	const tableRows = visibleRows
		.map((row, idx) => {
			const y = tableTop + 62 + idx * 42;
			const statusColor = row.status === "relayed" ? "#059669" : "#d97706";
			return `
			<line x1="40" y1="${y - 24}" x2="1240" y2="${y - 24}" stroke="#e5e7eb" />
			<text x="60" y="${y}" font-size="13" fill="#111827">${row.requestId}</text>
			<text x="160" y="${y}" font-size="13" fill="${statusColor}">${row.status}</text>
			<text x="300" y="${y}" font-size="13" fill="#111827">${row.latencySeconds ?? "-"}</text>
			<text x="430" y="${y}" font-size="13" fill="#111827">${escapeXml(shortHash(row.sourceTxHash))}</text>
			<text x="700" y="${y}" font-size="13" fill="#111827">${escapeXml(shortHash(row.relayTxHash))}</text>
			<text x="960" y="${y}" font-size="12" fill="#374151">${escapeXml(timestampToIso(row.sourceTimestamp))}</text>
			`;
		})
		.join("\n");

	return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
	<rect width="100%" height="100%" fill="#f8fafc"/>
	<text x="40" y="52" font-size="30" font-weight="700" fill="#0f172a">MPC-VDF E2E Monitoring Dashboard (Temporary)</text>
	<text x="40" y="82" font-size="14" fill="#475569">Generated at ${new Date().toISOString()}</text>

	<rect x="40" y="105" width="240" height="70" rx="12" fill="#e2e8f0"/>
	<rect x="300" y="105" width="240" height="70" rx="12" fill="#dcfce7"/>
	<rect x="560" y="105" width="240" height="70" rx="12" fill="#fef3c7"/>
	<rect x="820" y="105" width="240" height="70" rx="12" fill="#dbeafe"/>

	<text x="60" y="132" font-size="13" fill="#334155">Total Requests</text>
	<text x="60" y="158" font-size="24" font-weight="700" fill="#0f172a">${rows.length}</text>

	<text x="320" y="132" font-size="13" fill="#166534">Relayed</text>
	<text x="320" y="158" font-size="24" font-weight="700" fill="#166534">${relayed.length}</text>

	<text x="580" y="132" font-size="13" fill="#92400e">Pending</text>
	<text x="580" y="158" font-size="24" font-weight="700" fill="#92400e">${pending.length}</text>

	<text x="840" y="132" font-size="13" fill="#1d4ed8">Avg Latency (s)</text>
	<text x="840" y="158" font-size="24" font-weight="700" fill="#1e3a8a">${avgLatency}</text>

	<text x="40" y="220" font-size="18" font-weight="700" fill="#0f172a">Relay Latency by Request ID</text>
	<line x1="40" y1="490" x2="1240" y2="490" stroke="#94a3b8" />
	${bars}

	<text x="40" y="590" font-size="18" font-weight="700" fill="#0f172a">Latest Requests</text>
	<rect x="40" y="602" width="1200" height="34" fill="#e2e8f0" rx="8" />
	<text x="60" y="625" font-size="13" font-weight="700" fill="#0f172a">requestId</text>
	<text x="160" y="625" font-size="13" font-weight="700" fill="#0f172a">status</text>
	<text x="300" y="625" font-size="13" font-weight="700" fill="#0f172a">latency(s)</text>
	<text x="430" y="625" font-size="13" font-weight="700" fill="#0f172a">sourceTx</text>
	<text x="700" y="625" font-size="13" font-weight="700" fill="#0f172a">relayTx</text>
	<text x="960" y="625" font-size="13" font-weight="700" fill="#0f172a">sourceTime</text>
	${tableRows}
</svg>`;
}

async function main() {
	const sepoliaRpc = requiredEnv("SEPOLIA_RPC_URL");
	const destinationRpc = process.env.DEST_RPC_URL || requiredEnv("AMOY_RPC_URL");
	const expectedDestinationChainId = Number(process.env.DEST_CHAIN_ID || "80002");
	const routerAddress = process.env.RANDOM_ROUTER_ADDRESS || requiredEnv("RANDOM_SENDER_ADDRESS");
	const receiverAddress = requiredEnv("RANDOM_RECEIVER_ADDRESS");

	const lookback = Number(process.env.E2E_REPORT_LOOKBACK_BLOCKS || "150");
	const maxRange = Number(process.env.E2E_REPORT_MAX_BLOCK_RANGE || "10");

	const srcProvider = new ethers.JsonRpcProvider(sepoliaRpc);
	const dstProvider = new ethers.JsonRpcProvider(destinationRpc);

	const router = new ethers.Contract(routerAddress, ROUTER_ABI, srcProvider);
	const receiver = new ethers.Contract(receiverAddress, RECEIVER_ABI, dstProvider);

	const srcLatest = Number(await srcProvider.getBlockNumber());
	const destinationNetwork = await dstProvider.getNetwork();
	if (Number(destinationNetwork.chainId) !== expectedDestinationChainId) {
		throw new Error(
			`Destination chainId mismatch: expected ${expectedDestinationChainId}, got ${Number(destinationNetwork.chainId)}`,
		);
	}
	const dstLatest = Number(await dstProvider.getBlockNumber());
	const srcFrom = Math.max(0, srcLatest - lookback);
	const dstFrom = Math.max(0, dstLatest - lookback);

	const [srcLogs, dstLogs] = await Promise.all([
		queryEventsChunked(router, router.filters.LogRequest(), srcFrom, srcLatest, maxRange),
		queryEventsChunked(
			receiver,
			receiver.filters.OptimisticResultSubmitted(),
			dstFrom,
			dstLatest,
			maxRange,
		),
	]);

	const srcMap = new Map<number, E2ERow>();
	const dstMap = new Map<number, { relayTxHash: string; relayBlock: number }>();

	for (const log of srcLogs) {
		const requestId = toNum(log.args.requestId);
		srcMap.set(requestId, {
			requestId,
			sourceTxHash: log.transactionHash,
			sourceBlock: log.blockNumber,
			status: "pending",
		});
	}

	for (const log of dstLogs) {
		const requestId = toNum(log.args.requestId);
		dstMap.set(requestId, {
			relayTxHash: log.transactionHash,
			relayBlock: log.blockNumber,
		});
	}

	const allBlockTs = new Map<string, number>();
	const loadBlockTs = async (provider: ethers.JsonRpcProvider, chain: string, block?: number) => {
		if (!block) {
			return undefined;
		}
		const key = `${chain}:${block}`;
		if (!allBlockTs.has(key)) {
			const data = await provider.getBlock(block);
			if (!data) {
				return undefined;
			}
			allBlockTs.set(key, Number(data.timestamp));
		}
		return allBlockTs.get(key);
	};

	const rows: E2ERow[] = [];
	const requestIds = Array.from(srcMap.keys()).sort((a, b) => a - b);

	for (const requestId of requestIds) {
		const src = srcMap.get(requestId)!;
		const dst = dstMap.get(requestId);

		const sourceTimestamp = await loadBlockTs(srcProvider, "src", src.sourceBlock);
		const relayTimestamp = await loadBlockTs(dstProvider, "dst", dst?.relayBlock);
		const latencySeconds =
			sourceTimestamp && relayTimestamp ? Math.max(0, relayTimestamp - sourceTimestamp) : undefined;

		rows.push({
			requestId,
			sourceTxHash: src.sourceTxHash,
			sourceBlock: src.sourceBlock,
			sourceTimestamp,
			relayTxHash: dst?.relayTxHash,
			relayBlock: dst?.relayBlock,
			relayTimestamp,
			latencySeconds,
			status: dst ? "relayed" : "pending",
		});
	}

	const outDir = path.resolve(__dirname, "../../../docs");
	fs.mkdirSync(outDir, { recursive: true });

	const jsonPath = path.join(outDir, "e2e-monitor.json");
	const svgPath = path.join(outDir, "e2e-monitor.svg");

	const latencyValues = rows
		.filter((r) => r.status === "relayed" && typeof r.latencySeconds === "number")
		.map((r) => r.latencySeconds as number)
		.sort((a, b) => a - b);
	const latencySummary = {
		count: latencyValues.length,
		avgSeconds: latencyValues.length
			? Number((latencyValues.reduce((acc, x) => acc + x, 0) / latencyValues.length).toFixed(3))
			: 0,
		p50Seconds: Number(percentile(latencyValues, 50).toFixed(3)),
		p95Seconds: Number(percentile(latencyValues, 95).toFixed(3)),
	};

	fs.writeFileSync(
		jsonPath,
		JSON.stringify(
			{
				generatedAt: new Date().toISOString(),
				lookbackBlocks: lookback,
				sourceLatestBlock: srcLatest,
				destinationLatestBlock: dstLatest,
				destinationChainId: Number(destinationNetwork.chainId),
				latencySummary,
				rows,
			},
			null,
			2,
		),
	);

	fs.writeFileSync(svgPath, buildSvg(rows));

	const relayedCount = rows.filter((r) => r.status === "relayed").length;
	const pendingCount = rows.length - relayedCount;

	console.log(`E2E rows: ${rows.length}`);
	console.log(`Relayed: ${relayedCount}`);
	console.log(`Pending: ${pendingCount}`);
	console.log(`Avg latency(s): ${latencySummary.avgSeconds}`);
	console.log(`P50 latency(s): ${latencySummary.p50Seconds}`);
	console.log(`P95 latency(s): ${latencySummary.p95Seconds}`);
	console.log(`JSON: ${jsonPath}`);
	console.log(`SVG : ${svgPath}`);
}

main().catch((error) => {
	console.error(error);
	process.exitCode = 1;
});
