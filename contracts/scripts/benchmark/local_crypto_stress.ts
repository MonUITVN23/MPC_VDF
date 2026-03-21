import * as fs from "fs";
import * as path from "path";
import { spawnSync } from "child_process";
import * as dotenv from "dotenv";
import { ethers } from "hardhat";

const envCandidates = [
	path.resolve(__dirname, "../../.env"),
	path.resolve(__dirname, "../../../.env"),
];
const envPath = envCandidates.find((p) => fs.existsSync(p));
dotenv.config(envPath ? { path: envPath } : undefined);

type RustVdfSample = {
	t_value: number;
	prover_time_ms: number;
	seed_collective_hex: string;
	pi_hex: string;
};

function parseTValues(): number[] {
	const raw = process.env.BENCH_T_VALUES || "32768,262144,1048576";
	const values = raw
		.split(",")
		.map((v) => Number(v.trim()))
		.filter((v) => Number.isFinite(v) && v > 0);
	if (values.length === 0) {
		throw new Error("BENCH_T_VALUES is empty/invalid");
	}
	return values;
}

function parsePositiveInt(name: string, fallback: number): number {
	const raw = process.env[name];
	if (!raw) {
		return fallback;
	}
	const value = Number(raw);
	if (!Number.isFinite(value) || value <= 0) {
		throw new Error(`Invalid ${name}: ${raw}`);
	}
	return value;
}

function getModulusHex(): string {
	const envHex = process.env.VDF_MODULUS_HEX;
	if (envHex && envHex.length > 2) {
		return envHex;
	}
	const bytes = new Uint8Array(130);
	bytes.fill(0x11);
	bytes[bytes.length - 1] = 1;
	return ethers.hexlify(bytes);
}

function ensureCsvHeader(csvPath: string) {
	if (!fs.existsSync(csvPath) || fs.statSync(csvPath).size === 0) {
		fs.writeFileSync(csvPath, "T_value,prover_time_ms,verify_gas_used\n", { encoding: "utf-8" });
	}
}

function appendCsvRow(csvPath: string, tValue: number, proverTimeMs: number, verifyGasUsed: bigint) {
	fs.appendFileSync(csvPath, `${tValue},${proverTimeMs},${verifyGasUsed.toString()}\n`, {
		encoding: "utf-8",
	});
}

function runRustVdfSample(tValue: number, offchainDir: string): RustVdfSample {
	const sessionId = `hardhat-local-t${tValue}-${Date.now()}`;
	const result = spawnSync(
		"cargo",
		[
			"run",
			"-p",
			"network_module",
			"--bin",
			"vdf_pipeline_once",
			"--",
			"--t",
			String(tValue),
			"--session-id",
			sessionId,
		],
		{
			cwd: offchainDir,
			encoding: "utf-8",
		}
	);

	if (result.status !== 0) {
		throw new Error(`Rust VDF command failed (t=${tValue}): ${result.stderr || result.stdout}`);
	}

	const stdout = (result.stdout || "").trim();
	const lastJsonLine = stdout
		.split(/\r?\n/)
		.reverse()
		.find((line) => line.trim().startsWith("{"));

	if (!lastJsonLine) {
		throw new Error(`Rust output missing JSON line (t=${tValue}): ${stdout}`);
	}

	return JSON.parse(lastJsonLine) as RustVdfSample;
}

async function main() {
	const networkName = (await ethers.provider.getNetwork()).name;
	if (networkName !== "unknown") {
		console.log(`[local-crypto-stress] network=${networkName}`);
	}

	const tValues = parseTValues();
	const repeats = parsePositiveInt("BENCH_REPEATS_PER_T", 5);
	const repoRoot = path.resolve(__dirname, "../../..");
	const offchainDir = path.resolve(repoRoot, "off-chain");
	const csvPath = process.env.CRYPTO_BENCH_CSV_PATH || path.resolve(offchainDir, "crypto_benchmarks.csv");
	const modulusHex = getModulusHex();

	ensureCsvHeader(csvPath);

	const verifierFactory = await ethers.getContractFactory("VDFVerifierMock");
	const verifier = await verifierFactory.deploy();
	await verifier.waitForDeployment();

	const [signer] = await ethers.getSigners();
	console.log(`[local-crypto-stress] verifier=${await verifier.getAddress()}`);
	console.log(`[local-crypto-stress] csv=${csvPath}`);

	for (const tValue of tValues) {
		for (let run = 0; run < repeats; run++) {
			const sample = runRustVdfSample(tValue, offchainDir);

			const txReq = await verifier.verifyVDFPublic.populateTransaction(
				sample.seed_collective_hex,
				sample.pi_hex,
				modulusHex,
			);

			const tx = await signer.sendTransaction({
				to: await verifier.getAddress(),
				data: txReq.data,
			});
			const receipt = await tx.wait();
			if (!receipt) {
				throw new Error(`Missing receipt for t=${tValue}, run=${run}`);
			}

			appendCsvRow(csvPath, tValue, sample.prover_time_ms, receipt.gasUsed);
			console.log(
				`[local-crypto-stress] T=${tValue} run=${run} prover_ms=${sample.prover_time_ms} verify_gas=${receipt.gasUsed.toString()}`,
			);
		}
	}

	console.log("[local-crypto-stress] done");
}

main().catch((error) => {
	console.error(error);
	process.exitCode = 1;
});
