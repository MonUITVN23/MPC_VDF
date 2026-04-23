import { expect } from "chai";
import { ethers } from "hardhat";
import * as crypto from "crypto";

describe("E2E ZK Multi-Bridge Optimization & Gas Benchmark (Halo2)", function () {
  this.timeout(0); 

  let receiver: any;
  let zkVerifier: any;
  let owner: any;
  let adapter1: any;
  let adapter2: any;
  let adapter3: any;

  
  const pk = new Uint8Array(48).fill(1);
  const sig = new Uint8Array(96).fill(2);
  const msg = new Uint8Array(32).fill(3);
  const y = new Uint8Array(128).fill(4);
  const pi = new Uint8Array(128).fill(5);
  const modulus = new Uint8Array(32).fill(6);
  const requestId = 99999n;

  let zkProofData: string;
  let zkPublicSignals: any[];

  
  function sha256Split(data: Buffer): { hi: bigint; lo: bigint } {
    const hash = crypto.createHash("sha256").update(data).digest();
    const hi = BigInt("0x" + hash.subarray(0, 16).toString("hex"));
    const lo = BigInt("0x" + hash.subarray(16, 32).toString("hex"));
    return { hi, lo };
  }

  before(async function () {
    [owner, adapter1, adapter2, adapter3] = await ethers.getSigners();

    
    const VerifierFactory = await ethers.getContractFactory("Halo2Verifier");
    zkVerifier = await VerifierFactory.deploy();

    
    
    const ReceiverFactory = await ethers.getContractFactory("RandomReceiver");
    receiver = await ReceiverFactory.deploy(owner.address);

    
    const verifierAddr = await zkVerifier.getAddress();
    await receiver.setZkVerifier(verifierAddr);
    await receiver.setZkProofMode(true);

    
    
    
    console.log("    Generating Halo2 proof data for test...");

    
    const commitmentInput = Buffer.concat([Buffer.from(pk), Buffer.from(sig), Buffer.from(msg)]);
    const commitment = sha256Split(commitmentInput);

    const pkHash = sha256Split(Buffer.from(pk));

    const requestIdBuf = Buffer.alloc(32);
    requestIdBuf.writeBigUInt64BE(requestId, 24);
    const payloadInput = Buffer.concat([
      requestIdBuf, Buffer.from(y), Buffer.from(pi), Buffer.from(msg), Buffer.from(modulus)
    ]);
    const payloadHash = sha256Split(payloadInput);

    zkPublicSignals = [
      commitment.hi,
      commitment.lo,
      pkHash.hi,
      pkHash.lo,
      payloadHash.hi,
      payloadHash.lo,
      requestId,
    ];

    
    
    const proofBytes = Buffer.alloc(256);
    
    const g1x = BigInt(1);
    const g1y = BigInt(2);
    const xBuf = Buffer.alloc(32);
    const yBuf = Buffer.alloc(32);
    xBuf.writeBigUInt64BE(g1x, 24);
    yBuf.writeBigUInt64BE(g1y, 24);
    proofBytes.set(xBuf, 0);
    proofBytes.set(yBuf, 32);
    
    proofBytes.set(xBuf, 192);
    proofBytes.set(yBuf, 224);

    
    const AbiCoder = ethers.AbiCoder.defaultAbiCoder();
    zkProofData = AbiCoder.encode(["bytes"], [proofBytes]);

    
    const pkHashHex = "0x" + pkHash.hi.toString(16).padStart(32, "0") + pkHash.lo.toString(16).padStart(32, "0");
    await receiver.registerPkHash(pkHashHex);
  });

  it("Bridge 1 (Axelar Mock): Should verify Halo2 ZK proof and report gas", async function () {
    const tx = await receiver.connect(adapter1).submitOptimisticResult(
      requestId,
      y,
      pi,
      msg, 
      modulus,
      sig, 
      zkProofData,
      zkPublicSignals
    );

    const receipt = await tx.wait();
    console.log(`      Gas used for Halo2 Verification + Enqueue: ${receipt.gasUsed}`);

    const req = await receiver.queue(requestId);
    expect(req.submittedAtBlock).to.be.gt(0n);
  });

  it("Bridge 2 (LayerZero Mock): Should reject invalid proof payload", async function () {
    const invalidY = new Uint8Array(128).fill(9); 

    await expect(
      receiver.connect(adapter2).submitOptimisticResult(
        requestId + 1n,
        invalidY, 
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
    
    const AbiCoder = ethers.AbiCoder.defaultAbiCoder();
    const fakeProofData = AbiCoder.encode(["bytes"], [new Uint8Array(32)]); 

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
