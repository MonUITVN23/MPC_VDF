import { expect } from "chai";
import { ethers } from "hardhat";

describe("VDFVerifierMock", function () {
	it("compares gas between precompile and unoptimized path", async function () {
		const factory = await ethers.getContractFactory("VDFVerifierMock");
		const verifier = await factory.deploy();
		await verifier.waitForDeployment();

		const base = ethers.hexlify(ethers.randomBytes(32));
		const exponent = ethers.hexlify(ethers.randomBytes(130));
		const modulusBytes = ethers.randomBytes(130);
		modulusBytes[modulusBytes.length - 1] = 1;
		const modulus = ethers.hexlify(modulusBytes);

		const result = await verifier.verifyVDFPublic(base, exponent, modulus);
		expect(ethers.getBytes(result).length).to.equal((modulus.length - 2) / 2);

		const unoptimizedResult = await verifier.verifyVDF_Unoptimized(
			base,
			exponent,
			modulus
		);
		expect(ethers.getBytes(unoptimizedResult).length).to.equal((modulus.length - 2) / 2);

		const [signer] = await ethers.getSigners();
		const txRequestOptimized = await verifier.verifyVDFPublic.populateTransaction(
			base,
			exponent,
			modulus
		);
		const txRequestUnoptimized = await verifier.verifyVDF_Unoptimized.populateTransaction(
			base,
			exponent,
			modulus
		);

		const txOptimized = await signer.sendTransaction({
			to: await verifier.getAddress(),
			data: txRequestOptimized.data,
		});
		const receiptOptimized = await txOptimized.wait();

		const txUnoptimized = await signer.sendTransaction({
			to: await verifier.getAddress(),
			data: txRequestUnoptimized.data,
		});
		const receiptUnoptimized = await txUnoptimized.wait();

		console.log("gasUsed optimized:", receiptOptimized?.gasUsed?.toString() ?? "N/A");
		console.log("gasUsed unoptimized:", receiptUnoptimized?.gasUsed?.toString() ?? "N/A");
		expect(receiptOptimized).to.not.equal(null);
		expect(receiptUnoptimized).to.not.equal(null);
	});
});
