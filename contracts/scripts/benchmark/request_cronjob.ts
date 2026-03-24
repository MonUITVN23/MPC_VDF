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

const RANDOM_ROUTER_ABI = [
	"function requestRandomness(uint256 userSeed) external returns (uint256 requestId)",
	"event LogRequest(uint256 indexed requestId, uint256 userSeed, uint256 timestamp)",
];

function envRequired(name: string): string {
	const value = process.env[name];
	if (!value) {
		throw new Error(`Missing required env var: ${name}`);
	}
	return value;
}

function sleep(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms));
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

async function sendRequestWithRetry(
	sender: ethers.Contract,
	maxRetries: number,
	retryDelayMs: number,
): Promise<{ requestId?: bigint; txHash: string; sentAtIso: string }> {
	let lastError: unknown;

	for (let attempt = 1; attempt <= maxRetries; attempt++) {
		const sentAtIso = new Date().toISOString();
		const userSeed = BigInt(ethers.hexlify(ethers.randomBytes(32)));

		try {
			const tx = await sender.requestRandomness(userSeed);
			const receipt = await tx.wait(1);

			let requestId: bigint | undefined;
			for (const log of receipt.logs ?? []) {
				try {
					const parsed = sender.interface.parseLog(log);
					if (parsed && parsed.name === "LogRequest") {
						requestId = parsed.args.requestId as bigint;
						break;
					}
				} catch {
					continue;
				}
			}

			return {
				requestId,
				txHash: tx.hash,
				sentAtIso,
			};
		} catch (error) {
			lastError = error;
			console.warn(
				`[request-cronjob] attempt ${attempt}/${maxRetries} failed: ${String(error)}`,
			);
			if (attempt < maxRetries) {
				await sleep(retryDelayMs * attempt);
			}
		}
	}

	throw new Error(`[request-cronjob] all retries failed: ${String(lastError)}`);
}

async function main() {
	const sepoliaRpcUrl = envRequired("SEPOLIA_RPC_URL");
	const privateKey = envRequired("PRIVATE_KEY");
	const routerAddress = process.env.RANDOM_ROUTER_ADDRESS || envRequired("RANDOM_SENDER_ADDRESS");

	const intervalSeconds = parsePositiveInt("REQUEST_INTERVAL_SECONDS", 15);
	const maxRetries = parsePositiveInt("REQUEST_MAX_RETRIES", 5);
	const retryDelaySeconds = parsePositiveInt("REQUEST_RETRY_DELAY_SECS", 8);
	const maxRequests = Number(process.env.REQUEST_MAX_REQUESTS || "0");

	const provider = new ethers.JsonRpcProvider(sepoliaRpcUrl);
	const wallet = new ethers.Wallet(privateKey, provider);
	const router = new ethers.Contract(routerAddress, RANDOM_ROUTER_ABI, wallet);

	let sentCount = 0;
	console.log(
		`[request-cronjob] start interval=${intervalSeconds}s retries=${maxRetries} router=${routerAddress}`,
	);

	while (true) {
		try {
			const out = await sendRequestWithRetry(router, maxRetries, retryDelaySeconds * 1000);
			sentCount += 1;

			const requestLabel = out.requestId !== undefined ? out.requestId.toString() : `unknown-${sentCount}`;
			console.log(
				`[request-cronjob] Sent Request #${requestLabel} at ${out.sentAtIso} tx=${out.txHash}`,
			);
		} catch (error) {
			console.error(`[request-cronjob] send failed after retries: ${String(error)}`);
		}

		if (maxRequests > 0 && sentCount >= maxRequests) {
			console.log(`[request-cronjob] reached REQUEST_MAX_REQUESTS=${maxRequests}, stopping.`);
			break;
		}

		await sleep(intervalSeconds * 1000);
	}
}

main().catch((error) => {
	console.error(error);
	process.exitCode = 1;
});
