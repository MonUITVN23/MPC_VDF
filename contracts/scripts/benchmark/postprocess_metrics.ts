import * as fs from "fs";
import * as path from "path";

type Stats = {
	count: number;
	min: number;
	max: number;
	avg: number;
	p50: number;
	p95: number;
};

type CryptoRow = {
	T_value: number;
	prover_time_ms: number;
	proof_size_bytes: number;
	verify_gas_used: number;
};

type E2ERow = {
	request_id: number;
	bridge_id: number;
	t1_timestamp: number;
	t2_mpc_ms: number;
	t3_vdf_ms: number;
	t4_dispatch_ms: number;
	tx_hash: string;
};

function parseNumber(value: string, field: string, line: number): number {
	const num = Number(value);
	if (!Number.isFinite(num)) {
		throw new Error(`Invalid number for ${field} at CSV line ${line}: ${value}`);
	}
	return num;
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

function computeStats(values: number[]): Stats {
	if (values.length === 0) {
		return {
			count: 0,
			min: 0,
			max: 0,
			avg: 0,
			p50: 0,
			p95: 0,
		};
	}

	const sorted = [...values].sort((a, b) => a - b);
	const sum = sorted.reduce((acc, v) => acc + v, 0);

	return {
		count: sorted.length,
		min: sorted[0],
		max: sorted[sorted.length - 1],
		avg: sum / sorted.length,
		p50: percentile(sorted, 50),
		p95: percentile(sorted, 95),
	};
}

function readCsvLines(csvPath: string): string[] {
	if (!fs.existsSync(csvPath)) {
		console.warn(`[postprocess] CSV not found, skipping: ${csvPath}`);
		return [];
	}

	const raw = fs.readFileSync(csvPath, "utf-8").trim();
	if (!raw) {
		console.warn(`[postprocess] CSV empty, skipping: ${csvPath}`);
		return [];
	}

	return raw.split(/\r?\n/).filter((line) => line.trim().length > 0);
}

function parseCryptoCsv(csvPath: string): CryptoRow[] {
	const lines = readCsvLines(csvPath);
	if (lines.length <= 1) {
		return [];
	}

	const rows: CryptoRow[] = [];
	for (let i = 1; i < lines.length; i++) {
		const lineNo = i + 1;
		const [T_value, prover_time_ms, proof_size_bytes, verify_gas_used] = lines[i].split(",");
		if ([T_value, prover_time_ms, proof_size_bytes, verify_gas_used].some((v) => v === undefined)) {
			console.warn(`[postprocess] Skip malformed crypto CSV line ${lineNo}`);
			continue;
		}

		rows.push({
			T_value: parseNumber(T_value, "T_value", lineNo),
			prover_time_ms: parseNumber(prover_time_ms, "prover_time_ms", lineNo),
			proof_size_bytes: parseNumber(proof_size_bytes, "proof_size_bytes", lineNo),
			verify_gas_used: parseNumber(verify_gas_used, "verify_gas_used", lineNo),
		});
	}

	return rows;
}

function parseE2ECsv(csvPath: string): E2ERow[] {
	const lines = readCsvLines(csvPath);
	if (lines.length <= 1) {
		return [];
	}

	const rows: E2ERow[] = [];
	for (let i = 1; i < lines.length; i++) {
		const lineNo = i + 1;
		const [request_id, bridge_id, t1_timestamp, t2_mpc_ms, t3_vdf_ms, t4_dispatch_ms, tx_hash] =
			lines[i].split(",");

		if (
			[request_id, bridge_id, t1_timestamp, t2_mpc_ms, t3_vdf_ms, t4_dispatch_ms, tx_hash].some(
				(v) => v === undefined,
			)
		) {
			console.warn(`[postprocess] Skip malformed e2e CSV line ${lineNo}`);
			continue;
		}

		rows.push({
			request_id: parseNumber(request_id, "request_id", lineNo),
			bridge_id: parseNumber(bridge_id, "bridge_id", lineNo),
			t1_timestamp: parseNumber(t1_timestamp, "t1_timestamp", lineNo),
			t2_mpc_ms: parseNumber(t2_mpc_ms, "t2_mpc_ms", lineNo),
			t3_vdf_ms: parseNumber(t3_vdf_ms, "t3_vdf_ms", lineNo),
			t4_dispatch_ms: parseNumber(t4_dispatch_ms, "t4_dispatch_ms", lineNo),
			tx_hash: tx_hash.trim(),
		});
	}

	return rows;
}

function groupBy<T>(rows: T[], keyFn: (r: T) => string): Record<string, T[]> {
	const out: Record<string, T[]> = {};
	for (const row of rows) {
		const key = keyFn(row);
		if (!out[key]) {
			out[key] = [];
		}
		out[key].push(row);
	}
	return out;
}

function buildCryptoSummary(cryptoRows: CryptoRow[], inputPath: string) {
	const grouped = groupBy(cryptoRows, (r) => String(r.T_value));
	const byTValue: Record<string, unknown> = {};

	for (const [tValue, rows] of Object.entries(grouped)) {
		byTValue[tValue] = {
			count: rows.length,
			prover_time_ms: computeStats(rows.map((r) => r.prover_time_ms)),
			verify_gas_used: computeStats(rows.map((r) => r.verify_gas_used)),
			proof_size_bytes: computeStats(rows.map((r) => r.proof_size_bytes)),
		};
	}

	return {
		generated_at: new Date().toISOString(),
		input_csv: inputPath,
		total_rows: cryptoRows.length,
		groups: byTValue,
	};
}

function buildE2ESummary(e2eRows: E2ERow[], inputPath: string) {
	const grouped = groupBy(e2eRows, (r) => String(r.bridge_id));
	const byBridge: Record<string, unknown> = {};

	for (const [bridgeId, rows] of Object.entries(grouped)) {
		byBridge[bridgeId] = {
			count: rows.length,
			t1_to_t4_dispatch_latency_ms: computeStats(rows.map((r) => r.t4_dispatch_ms)),
			t2_mpc_ms: computeStats(rows.map((r) => r.t2_mpc_ms)),
			t3_vdf_ms: computeStats(rows.map((r) => r.t3_vdf_ms)),
		};
	}

	return {
		generated_at: new Date().toISOString(),
		input_csv: inputPath,
		total_rows: e2eRows.length,
		groups: byBridge,
	};
}

function main() {
	const repoRoot = path.resolve(__dirname, "../../..");
	const defaultCryptoCsv = path.resolve(repoRoot, "off-chain/crypto_benchmarks.csv");
	const defaultE2ECsv = path.resolve(repoRoot, "off-chain/e2e_metrics.csv");

	const cryptoCsvPath = process.env.CRYPTO_CSV_PATH || defaultCryptoCsv;
	const e2eCsvPath = process.env.E2E_CSV_PATH || defaultE2ECsv;

	const outputDir = process.env.SUMMARY_OUTPUT_DIR || path.resolve(repoRoot, "off-chain");
	const cryptoOutPath = path.resolve(outputDir, "crypto_summary.json");
	const e2eOutPath = path.resolve(outputDir, "e2e_summary.json");

	fs.mkdirSync(outputDir, { recursive: true });

	const cryptoRows = parseCryptoCsv(cryptoCsvPath);
	const e2eRows = parseE2ECsv(e2eCsvPath);

	const cryptoSummary = buildCryptoSummary(cryptoRows, cryptoCsvPath);
	const e2eSummary = buildE2ESummary(e2eRows, e2eCsvPath);

	fs.writeFileSync(cryptoOutPath, JSON.stringify(cryptoSummary, null, 2));
	fs.writeFileSync(e2eOutPath, JSON.stringify(e2eSummary, null, 2));

	console.log(`[postprocess] wrote ${cryptoOutPath}`);
	console.log(`[postprocess] wrote ${e2eOutPath}`);
	console.log(
		`[postprocess] rows: crypto=${cryptoRows.length}, e2e=${e2eRows.length}`,
	);
}

try {
	main();
} catch (error) {
	console.error(error);
	process.exitCode = 1;
}
