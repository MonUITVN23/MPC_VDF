import { expect } from "chai";
import { ethers } from "hardhat";
import { execSync } from "child_process";
import * as fs from "fs";
import * as path from "path";

describe("E2E ZK Multi-Bridge Optimization & Gas Benchmark", function () {
  this.timeout(0); // Disable timeout since ZK proving takes time

  let receiver: any;
  let zkVerifier: any;
  let owner: any;
  let adapter1: any;
  let adapter2: any;
  let adapter3: any;

  // Dummy inputs for ZK
  const pk = new Uint8Array(48).fill(1);
  const sig = new Uint8Array(96).fill(2);
  const msg = new Uint8Array(32).fill(3);
  const y = new Uint8Array(128).fill(4);
  const pi = new Uint8Array(128).fill(5);
  const modulus = new Uint8Array(32).fill(6);
  const requestId = 99999n;

  let zkProofData: string;
  let zkPublicSignals: any[];

  before(async function () {
    [owner, adapter1, adapter2, adapter3] = await ethers.getSigners();

    // 1. Deploy Groth16 Verifier
    const VerifierFactory = await ethers.getContractFactory("Groth16Verifier");
    zkVerifier = await VerifierFactory.deploy();

    // 2. Deploy Receiver
    // We mock the Axelar Gateway as the owner just to allow deployment
    const ReceiverFactory = await ethers.getContractFactory("RandomReceiver");
    receiver = await ReceiverFactory.deploy(owner.address);

    // Config Receiver
    const verifierAddr = await zkVerifier.getAddress();
    await receiver.setZkVerifier(verifierAddr);
    await receiver.setZkProofMode(true);

    // Generate ZK Proof using the external node script
    console.log("    Generating real ZK Proof using snarkjs (this may take a few seconds)...");
    
    const inputObj = {
      pk: Buffer.from(pk).toString('hex'),
      sig: Buffer.from(sig).toString('hex'),
      msg: Buffer.from(msg).toString('hex'),
      y: Buffer.from(y).toString('hex'),
      pi: Buffer.from(pi).toString('hex'),
      modulus: Buffer.from(modulus).toString('hex'),
      requestId: requestId.toString()
    };

    const tempDir = path.join(__dirname, `../circuits/temp_test_${Date.now()}`);
    fs.mkdirSync(tempDir, { recursive: true });
    
    const inputPath = path.join(tempDir, "input.json");
    fs.writeFileSync(inputPath, JSON.stringify(inputObj));

    const proveScript = path.join(__dirname, "../circuits/scripts/prove.js");
    execSync(`node ${proveScript} --input ${inputPath} --output ${tempDir}`);

    const proofStr = fs.readFileSync(path.join(tempDir, "proof.json"), "utf8");
    const publicStr = fs.readFileSync(path.join(tempDir, "public.json"), "utf8");
    
    const proof = JSON.parse(proofStr);
    const pubSignals = JSON.parse(publicStr);

    zkPublicSignals = pubSignals;

    // ABI Encode Proof
    const AbiCoder = ethers.AbiCoder.defaultAbiCoder();
    zkProofData = AbiCoder.encode(
      ["uint256[2]", "uint256[2][2]", "uint256[2]"],
      [
        [proof.pi_a[0], proof.pi_a[1]],
        [
          [proof.pi_b[0][1], proof.pi_b[0][0]], // SnarkJS reverses pB
          [proof.pi_b[1][1], proof.pi_b[1][0]]
        ],
        [proof.pi_c[0], proof.pi_c[1]]
      ]
    );

    // Register correct pkHash based on the generated public signals
    const expectedPkHash = ethers.zeroPadValue("0x" + BigInt(pubSignals[2]).toString(16), 32);
    await receiver.registerPkHash(expectedPkHash);

    // Cleanup
    fs.rmSync(tempDir, { recursive: true, force: true });
  });

  it("Bridge 1 (Axelar Mock): Should verify ZK proof and report gas", async function () {
    const tx = await receiver.connect(adapter1).submitOptimisticResult(
      requestId,
      y,
      pi,
      msg, // seedCollective
      modulus,
      sig, // blsSignature
      zkProofData,
      zkPublicSignals
    );
    
    const receipt = await tx.wait();
    console.log(`      Gas used for ZK Verification + Enqueue: ${receipt.gasUsed}`);
    
    const req = await receiver.queue(requestId);
    expect(req.submittedAt).to.be.gt(0);
  });

  it("Bridge 2 (LayerZero Mock): Should reject invalid proof payload", async function () {
    const invalidY = new Uint8Array(128).fill(9); // Tampered payload
    
    await expect(
      receiver.connect(adapter2).submitOptimisticResult(
        requestId + 1n,
        invalidY, // Tampered
        pi,
        msg,
        modulus,
        sig,
        zkProofData,
        zkPublicSignals
      )
    ).to.be.revertedWithCustomError(receiver, "PayloadHashMismatch");
  });

  it("Bridge 3 (Wormhole Mock): Should reject invalid ZK proof", async function () {
    const fakeProofData = ethers.AbiCoder.defaultAbiCoder().encode(
      ["uint256[2]", "uint256[2][2]", "uint256[2]"],
      [
        [1, 2],
        [[3, 4], [5, 6]],
        [7, 8]
      ]
    );

    await expect(
      receiver.connect(adapter3).submitOptimisticResult(
        requestId + 2n,
        y,
        pi,
        msg,
        modulus,
        sig,
        fakeProofData,
        zkPublicSignals
      )
    ).to.be.revertedWithCustomError(receiver, "InvalidZkProof");
  });
});
