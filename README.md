# CrossRand: Hybrid ZK-MPC-VDF Cross-Chain Verifiable Randomness

A Proof-of-Concept for **secure, unbiased, cross-chain verifiable randomness** using a defense-in-depth architecture combining **MPC (BLS12-381 threshold)**, **VDF (Imaginary Quadratic Class Groups)**, and **ZK-SNARK (Halo2/IPA — No Trusted Setup)**, delivered across chains through a **multi-bridge router + adapter** layer (Axelar / LayerZero / Wormhole).

> Academic research prototype — designed for reproducible benchmarking (IEEE style plots).

---

## 1. What Changed Technically (so với README cũ)

Bản README trước mô tả kiến trúc tổng thể nhưng chưa phản ánh đúng trạng thái code hiện tại. Các thay đổi chính trong repo (xem `git log`):

| Khu vực | Trạng thái cũ (README cũ) | Trạng thái mới (code hiện tại) |
|---|---|---|
| **VDF difficulty** | T cố định (`2^18`) | **Adaptive VDF difficulty** — commit `a2060f0`; T điều chỉnh theo cấu hình runtime thay vì hardcode |
| **ZK stack** | Legacy trusted-setup stack (deprecated) | **Halo2/IPA (No Trusted Setup)** — Rust native prover in [off-chain/halo2_prover/](off-chain/halo2_prover/). Circuit: `BlsCommitmentCircuit` (~200 constraints). Uses Pasta curves with IPA commitments — fully transparent, no ceremony. |
| **ZK prover invocation** | Rust gọi `prove.js` qua Node.js subprocess | **Native Rust** — `crypto_engine` gọi trực tiếp `halo2_prover::prove()`. Không cần Node.js, snarkjs, circom. |
| **Bridge layer** | Router đơn giản | Router + adapter pattern hoàn chỉnh: [RandomRouter.sol](contracts/src/RandomRouter.sol), adapters trong [contracts/src/adapters/](contracts/src/adapters/) (Axelar / LayerZero / Wormhole), kèm **bridge_registry** + **failover** ở off-chain ([network_module/src/bridges.rs](off-chain/network_module/src/bridges.rs), `bridge_registry.rs`, `relayer_factory.rs`) |
| **Relayer architecture** | 1 binary monolithic | Tách binary: `vdf_pipeline_once`, `local_stress_benchmark`, `dummy_relayer_smoke` trong [network_module/src/bin/](off-chain/network_module/src/bin/); relayer plug-in qua `relayers/` module |
| **On-chain verifier** | Legacy verifier path (deprecated) | **Halo2Verifier.sol** — verifier dùng BN254 pairing precompiles, nhận `bytes proof` + `uint256[7] pubSignals`. Không cần trusted setup ceremony. |
| **Benchmark suite** | Script rời rạc | 5 kịch bản chuẩn hoá + chart IEEE: [scripts/benchmark/](scripts/benchmark/) và [scripts/plot/](scripts/plot/); thêm so sánh baseline **Chainlink VRF + Drand** (commit `e465fc1`) |
| **Metrics log** | `e2e_metrics.csv` | **`e2e_metrics_v2.csv`** có cột `t3_5_zkprove_ms` và `bridge_name`; gas metrics tách qua `contracts/scripts/benchmark/02_gas_metrics.ts` |
| **MPC setup** | Chỉ in-process | Có sẵn **docker-compose** 3-of-4 cluster ([docker/docker-compose.yml](docker/docker-compose.yml)) cho stress test |
| **Contracts tests** | Vài file Sender/Receiver | Thêm **`E2E_MultiBridge_ZK.test.ts`** phủ luồng router + ZK verify đầu-cuối |

Những phần KHÔNG thay đổi (giữ guardrail): MPC + VDF dual-security thesis, IQCG VDF (không dùng RSA), tách `crypto_engine` vs `network_module`, optimistic challenge-window pattern.

---

## 2. Architecture (current)

```
Source (Sepolia)                  Off-chain Relayer                 Destination (Polygon Amoy)
┌──────────────────┐   event     ┌──────────────────────────┐   bridge    ┌──────────────────────┐
│ RandomRouter.sol │────────────▶│ ① MPC (BLS12-381, t-of-n)│────────────▶│ RandomReceiver.sol   │
│  + Adapters      │             │ ② VDF IQCG  y=x^(2^T)    │             │  Halo2Verifier       │
│  (Axelar/LZ/WH)  │             │ ③ Halo2/IPA prove (Rust) │             │  optimistic enqueue  │
└──────────────────┘             │ ④ Router adapter dispatch│             │  challenge → finalize│
                                 └──────────────────────────┘             └──────────────────────┘
```

Security layers: **MPC** chống input-bias · **VDF** chống front-running · **ZK-SNARK (Halo2, no trusted setup)** giải quyết mismatch BLS12-381 ↔ BN254 · **Optimistic verify** giảm gas user xuống ~116k.

---

## 3. Project Layout

```
contracts/          Hardhat project
  src/              RandomRouter, RandomReceiver, VDFVerifier, Halo2Verifier, adapters/, interfaces/
  circuits/         bls_commitment.circom (legacy) + scripts/{setup.sh, prove.js, verify.js}
  scripts/          deploy/, ops/ (init/relay/finalize), benchmark/
  test/             E2E_MultiBridge_ZK, RandomReceiver, RandomSender, VDFVerifier
off-chain/          Rust workspace
  crypto_engine/    mpc/, vdf/, dkg/, Halo2 integration (generate_zk_proof)
  halo2_prover/     BlsCommitmentCircuit, prove/verify (IPA, no trusted setup)
  network_module/   main.rs, bridges.rs, bridge_registry.rs, relayer_factory.rs, relayers/, bin/*
scripts/
  benchmark/        01..05 benchmark scenarios + data/
  plot/             IEEE-style matplotlib plots
docker/             3-of-4 MPC cluster compose
```

---

## 4. Prerequisites

- Node.js 18+, npm
- Rust 1.70+ (stable)
- Python 3.10+ with `matplotlib pandas seaborn numpy` (plots only)
- (No longer needed) ~~circom 2.x + snarkjs~~ — replaced by native Rust Halo2 prover
- (Optional) Foundry `cast` for sending tx on testnets
- (Optional) Sepolia + Polygon Amoy RPC URLs + funded key for on-chain demo

---

## 5. Quick Test — 1–2 testcases nhỏ (không cần stress 100 runs)

Mục tiêu của phần này: xác nhận pipeline hoạt động end-to-end bằng **2 testcase tối thiểu**. Không cần testnet, chạy hoàn toàn local.

### 5.1 Testcase A — Halo2 ZK proof (native Rust, no trusted setup)

```bash
cd off-chain
# Run Halo2 prover unit tests (proves + verifies, ~7s)
cargo test -p halo2_prover -- --nocapture
```

Kỳ vọng: 6/6 tests pass. Proof generation + verification hoàn toàn bằng Rust, không cần Node.js, snarkjs, hay trusted setup ceremony.

### 5.2 Testcase B — Full pipeline once (MPC → VDF → ZK → Receiver) trên Hardhat local

```bash
# Terminal 1 — Hardhat local node
cd contracts
npx hardhat compile
npx hardhat node
```

```bash
# Terminal 2 — chạy 1 lần pipeline end-to-end
cd off-chain
# T nhỏ để test nhanh (~5s VDF thay vì ~29s mặc định)
VDF_T=65536 RUST_LOG=info cargo run --bin vdf_pipeline_once --release
```

Kỳ vọng:
- Log off-chain in tuần tự: `MPC sign OK → VDF eval ... ms → ZK prove ... ms → bridge dispatch OK`.
- File `off-chain/e2e_metrics_v2.csv` được append 1 dòng mới có `t3_5_zkprove_ms` và `bridge_name`.
- Hardhat node log `Halo2Verifier.verifyProof` = true và `RandomReceiver` phát event finalize sau challenge window.

### 5.3 (Optional) Unit tests Solidity — 2 test mẫu

```bash
cd contracts
npx hardhat test test/VDFVerifier.test.ts
npx hardhat test test/E2E_MultiBridge_ZK.test.ts
```

Chỉ cần 2 file này pass là đã cover: VDF verify on-chain + luồng router/adapter/receiver + Halo2 verify.

### 5.4 (Optional) Rust crypto smoke

```bash
cd off-chain
cargo test -p crypto_engine mpc::                # ~1 MPC 3-of-4 test
cargo test -p crypto_engine vdf:: -- --nocapture # ~1 VDF eval+verify test (T nhỏ)
```

---

## 6. Going Further (không bắt buộc cho smoke test)

- **Testnet demo:** `contracts/scripts/deploy/*` để deploy Router (Sepolia) + Receiver (Amoy), sau đó `scripts/ops/00_init_request.ts` → `01_relay_payload.ts` → `02_finalize_randomness.ts`.
- **Full benchmark suite (IEEE charts):** xem [scripts/README_benchmark.md](scripts/README_benchmark.md). Các script đều hỗ trợ cờ `--quick` để chạy phiên bản rút gọn (2 điểm / 2 runs) trước khi chạy full.
- **MPC 3-of-4 cluster:** `cd docker && VDF_T=65536 docker-compose up -d`.

---

## 7. Design Decisions (giữ nguyên)

| Decision | Rationale |
|---|---|
| Halo2/IPA native prover thay zkVM nặng | Không cần trusted setup, chạy Rust-native, benchmark ổn định trên máy phổ thông |
| IQCG VDF (không RSA) | Không cần trusted setup cho discriminant; fully decentralized |
| Bridge adapter pattern | Đổi transport layer không phải audit lại router core |
| Optimistic verification | Happy-path gas giảm ~62%; verify đắt chỉ chạy khi bị challenge |

## 8. Future Work

- Decentralized MPC TSS thật (hiện tại PoC vẫn nặng simulation ở 1 node)
- Tier-2 circom-pairing BLS12-381 đầy đủ (~16M constraints)
- Frontend DApp + subgraph indexer
- Hardware-accelerated VDF (ASIC/FPGA)

## License & Acknowledgments

Academic research prototype. Built on Halo2, vdf-rs, bls-signatures, Hardhat, Axelar, LayerZero, Wormhole.
