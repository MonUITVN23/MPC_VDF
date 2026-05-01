# CrossRand — Hybrid ZK-MPC-VDF Cross-Chain Verifiable Randomness

**CrossRand** is a proof-of-concept protocol that provides **secure, unbiased, and unpredictable randomness** for decentralized applications across multiple blockchains. It combines three cryptographic primitives — **Multi-Party Computation (MPC)**, **Verifiable Delay Functions (VDF)**, and **Zero-Knowledge Proofs (ZK-SNARK)** — into a defense-in-depth architecture, and delivers randomness across chains through a **multi-bridge router + adapter** layer supporting Axelar, LayerZero, and Wormhole.

> **Academic research prototype** — designed for reproducible benchmarking with IEEE-style publication outputs.

---

## Table of Contents

- [Why CrossRand?](#why-crossrand)
- [How It Works](#how-it-works)
- [Architecture Overview](#architecture-overview)
- [End-to-End Workflow](#end-to-end-workflow)
- [Project Structure](#project-structure)
- [Technology Stack](#technology-stack)
- [Prerequisites](#prerequisites)
- [Getting Started](#getting-started)
- [Running Tests](#running-tests)
- [Benchmarking](#benchmarking)
- [Key Design Decisions](#key-design-decisions)
- [Future Work](#future-work)
- [License & Acknowledgments](#license--acknowledgments)

---

## Why CrossRand?

Generating **trustworthy randomness** on a blockchain is an unsolved problem known as the *Randomness Paradox*: any on-chain random number can be predicted or manipulated by miners, validators, or the parties who generate it. Existing solutions (e.g., Chainlink VRF, drand) typically rely on a single trust assumption or a single transport layer.

CrossRand addresses this with a **triple-layer defense**:

| Layer | Threat Eliminated | How |
|---|---|---|
| **MPC** (BLS12-381 threshold signatures) | **Input Bias** — a minority coalition trying to skew the seed | A committee of `t-of-n` nodes independently generate shares; no single party can control the aggregate seed |
| **VDF** (Imaginary Quadratic Class Groups) | **Front-running** — knowing the result before it's finalized | A strictly sequential computation forces a time delay before the output is revealed; parallelism cannot speed it up |
| **ZK-SNARK** (Halo2/IPA — no trusted setup) | **Curve mismatch** — BLS12-381 signatures can't be cheaply verified on BN254 EVM chains | A zero-knowledge proof bridges the gap, proving cryptographic validity on-chain without expensive pairing operations |

On top of these layers, an **Optimistic Verification** model drastically reduces gas costs: the expensive on-chain verification only runs if someone challenges a submitted result during a time window.

---

## How It Works

At a high level, a randomness request flows through four stages:

```
  ┌─────────────┐       ┌──────────────────────┐       ┌────────────────────┐       ┌───────────────────┐
  │  1. REQUEST  │──────▶│  2. GENERATE & PROVE │──────▶│  3. BRIDGE / RELAY │──────▶│  4. VERIFY & USE  │
  │  (on-chain)  │       │    (off-chain)       │       │  (cross-chain)     │       │  (destination)    │
  └─────────────┘       └──────────────────────┘       └────────────────────┘       └───────────────────┘
   User calls            MPC → VDF → ZK Proof           Router dispatches           Optimistic queue →
   requestRandomness()   in Rust                        via Axelar/LZ/WH            Challenge window →
                                                                                     Finalize
```

1. **Request** — A user or dApp calls `requestRandomness(seed)` on `RandomRouter.sol` (source chain). The contract emits a `LogRequest` event.
2. **Generate & Prove** — Off-chain Rust nodes pick up the event and execute the cryptographic pipeline:
   - **MPC**: Threshold BLS12-381 signing produces `seed_collective`
   - **VDF**: Sequential squaring `y = x^(2^T)` over an imaginary quadratic class group yields the delayed output and a Wesolowski proof
   - **ZK**: A Halo2/IPA proof binds the BLS signature to the payload hash, making it verifiable on EVM
3. **Bridge** — The relayer node calls `relayVDFPayload()` on the Router, which dispatches the payload through a configured bridge adapter (Axelar, LayerZero, or Wormhole).
4. **Verify & Use** — The destination chain's `RandomReceiver.sol` verifies the Halo2 ZK proof, enqueues the result optimistically, and opens a challenge window. If unchallenged, the randomness is finalized and available to the consuming dApp.

---

## Architecture Overview

```
Source Chain (Sepolia)            Off-chain Relayer (Rust)           Destination Chain (Polygon Amoy)
┌───────────────────┐   event    ┌───────────────────────────┐  bridge   ┌───────────────────────┐
│  RandomRouter.sol │───────────▶│ ① MPC (BLS12-381, t-of-n) │──────────▶│  RandomReceiver.sol   │
│    + Adapters     │            │ ② VDF (IQCG, y = x^2^T)   │          │   Halo2Verifier.sol   │
│  (Axelar/LZ/WH)  │            │ ③ Halo2/IPA prove (Rust)   │          │   Optimistic enqueue  │
└───────────────────┘            │ ④ Router adapter dispatch  │          │   Challenge → Finalize│
                                 └───────────────────────────┘          └───────────────────────┘
```

### Security Layers at a Glance

- **MPC** prevents input bias — no single party controls the seed
- **VDF** prevents front-running — the result can't be known before the delay elapses
- **ZK-SNARK (Halo2, no trusted setup)** resolves the BLS12-381 ↔ BN254 curve mismatch for on-chain verification
- **Optimistic verification** reduces happy-path gas to ~116k (vs. full verification at ~500k+)

---

## End-to-End Workflow

![End-to-End Workflow](./assets/The_High_Level_Architecture.png)

---

## Project Structure

```
mpc-vdf/
├── contracts/                      # Solidity smart contracts (Hardhat)
│   ├── src/
│   │   ├── RandomRouter.sol        # Source-side entry point & bridge dispatcher
│   │   ├── RandomReceiver.sol      # Destination-side optimistic queue & finalization
│   │   ├── VDFVerifier.sol         # On-chain VDF verification (0x05 modexp precompile)
│   │   ├── Halo2Verifier.sol       # On-chain Halo2/IPA ZK proof verification
│   │   ├── adapters/               # Bridge adapters (Axelar, LayerZero, Wormhole)
│   │   ├── interfaces/             # IBridgeAdapter, ITransparentVerifier, etc.
│   │   └── mock/                   # Mock contracts for testing
│   ├── scripts/
│   │   ├── deploy/                 # Deployment scripts for testnets
│   │   ├── ops/                    # Operational scripts (init, relay, finalize)
│   │   └── benchmark/              # On-chain gas benchmarking
│   └── test/                       # Hardhat test suite
│       ├── E2E_MultiBridge_ZK.test.ts   # Full E2E: Router → Adapter → Receiver → ZK
│       └── VDFVerifier.test.ts          # VDF on-chain verification tests
│
├── off-chain/                      # Rust workspace
│   ├── crypto_engine/              # Pure cryptography library (no blockchain I/O)
│   │   ├── src/mpc/                # BLS12-381 threshold signature scheme
│   │   ├── src/vdf/                # IQCG VDF: sequential squaring + Wesolowski proof
│   │   └── src/dkg/                # Distributed Key Generation
│   ├── halo2_prover/               # Halo2/IPA ZK prover (no trusted setup)
│   │   ├── src/circuit.rs          # BlsCommitmentCircuit definition
│   │   └── src/prover.rs           # Prove & verify API
│   └── network_module/             # Async networking, RPC, and relay logic
│       ├── src/main.rs             # Daemon loop: polls events, drives pipeline
│       ├── src/bridges.rs          # MultiBridgeRouter with failover
│       ├── src/bridge_registry.rs  # Bridge registration & priority
│       ├── src/relayer_factory.rs  # Relayer instantiation
│       ├── src/relayers/           # Per-bridge relayer implementations
│       ├── src/rpc/                # Ethereum RPC interaction (ethers-rs)
│       └── src/bin/                # Standalone binaries
│           ├── vdf_pipeline_once.rs        # Run one full pipeline cycle
│           ├── local_stress_benchmark.rs   # Local stress testing
│           └── dummy_relayer_smoke.rs      # Smoke test for relayer
│
├── scripts/
│   ├── benchmark/                  # 5 standardized benchmark scenarios
│   │   ├── 00_clean_outputs.sh     # Reset benchmark data
│   │   ├── 01_offchain_compute.sh  # VDF scaling + ZK + CPU/RAM profiling
│   │   ├── 02_gas_metrics.ts       # On-chain gas cost measurement
│   │   ├── 03_latency_breakdown.sh # End-to-end latency (5 phases)
│   │   ├── 04_failover_test.sh     # Cross-chain bridge failover
│   │   ├── 05_mev_censorship.ts    # MEV/censorship resistance
│   │   └── data/                   # CSV outputs + generated charts
│   └── plot/                       # IEEE-style matplotlib plotting
│       ├── ieee_style.py           # Shared IEEE formatting config
│       ├── plot_offchain_compute.py
│       ├── plot_gas_metrics.py
│       ├── plot_latency_breakdown.py
│       ├── plot_failover.py
│       └── plot_mev_censorship.py
│
├── docker/                         # Docker Compose for MPC cluster
│   └── docker-compose.yml          # 3-of-4 MPC node cluster
│
├── test_vectors.json               # Pre-computed test vectors for deterministic testing
├── Architecture.md                 # Detailed architecture documentation
└── README.md                       # This file
```

---

## Technology Stack

| Component | Technology |
|---|---|
| Smart Contracts | Solidity (Hardhat framework) |
| ZK Proving | Halo2/IPA in Rust — transparent, no trusted setup |
| Off-chain Crypto | Rust (`bls-signatures`, `vdf-rs` with IQCG groups) |
| Off-chain Networking | Rust async (`ethers-rs`, `tokio`) |
| Blockchain Networks | Ethereum Sepolia (source), Polygon Amoy (destination) |
| Cross-chain Bridges | Axelar, LayerZero, Wormhole (via adapter pattern) |
| Benchmarking & Plots | Bash, TypeScript, Python (`matplotlib`, `pandas`, `seaborn`) |
| MPC Cluster | Docker Compose (3-of-4 threshold default) |

---

## Prerequisites

| Requirement | Version | Notes |
|---|---|---|
| **Node.js** | 18+ | Required for Hardhat and contract scripts |
| **npm** | 9+ | Comes with Node.js |
| **Rust** | 1.70+ (stable) | For off-chain crypto engine and Halo2 prover |
| **Python** | 3.10+ | Only needed for benchmark plot generation |

**Python packages** (plots only):
```bash
pip install matplotlib pandas seaborn numpy
```

**No longer needed:**
- ~~Circom 2.x~~ — replaced by native Rust Halo2 prover
- ~~snarkjs~~ — replaced by native Rust Halo2 prover
- ~~Node.js prove.js~~ — ZK proving is now fully Rust-native

**Optional:**
- [Foundry](https://book.getfoundry.sh/) (`cast`) for sending transactions on testnets
- Sepolia + Polygon Amoy RPC URLs and a funded wallet for live cross-chain demos

---

## Getting Started

### 1. Clone the Repository

```bash
git clone https://github.com/MonUITVN23/MPC_VDF.git
cd MPC_VDF
```

### 2. Install Dependencies

```bash
# Smart contracts
cd contracts && npm install && cd ..

# Rust off-chain components
cd off-chain && cargo build --release && cd ..
```

### 3. Compile Smart Contracts

```bash
cd contracts
npx hardhat compile
```

### 4. Run a Quick Smoke Test

This verifies the entire pipeline works end-to-end on your local machine — no testnet required.

**Terminal 1** — Start a local Hardhat node:
```bash
cd contracts
npx hardhat node
```

**Terminal 2** — Run one full pipeline cycle:
```bash
cd off-chain
VDF_T=65536 RUST_LOG=info cargo run --bin vdf_pipeline_once --release
```

You should see sequential log output:
```
MPC sign OK → VDF eval ... ms → ZK prove ... ms → bridge dispatch OK
```

A new row will be appended to `off-chain/e2e_metrics_v2.csv` with timing data for each phase.

---

## Running Tests

### Halo2 ZK Prover (Rust)

```bash
cd off-chain
cargo test -p halo2_prover -- --nocapture
```

Runs 6 tests covering proof generation and verification — fully in Rust, no external dependencies.

### MPC & VDF Crypto Engine (Rust)

```bash
cd off-chain
cargo test -p crypto_engine mpc::                # MPC 3-of-4 threshold test
cargo test -p crypto_engine vdf:: -- --nocapture  # VDF eval + verify test
```

### Smart Contracts (Solidity)

```bash
cd contracts

# VDF on-chain verification
npx hardhat test test/VDFVerifier.test.ts

# Full E2E: Router → Bridge Adapter → Receiver → Halo2 ZK Verify
npx hardhat test test/E2E_MultiBridge_ZK.test.ts
```

---

## Benchmarking

CrossRand includes a standardized 5-scenario benchmarking suite producing IEEE-quality figures.

### Quick Overview

| # | Scenario | Script | What It Measures |
|---|---|---|---|
| 1 | Off-chain Compute | `01_offchain_compute.sh` | VDF scaling (2^16 → 2^24), ZK proving time, CPU/RAM |
| 2 | Gas Economics | `02_gas_metrics.ts` | On-chain gas costs (optimistic vs. pessimistic vs. baseline) |
| 3 | Latency Breakdown | `03_latency_breakdown.sh` | End-to-end latency split into 5 phases |
| 4 | Bridge Failover | `04_failover_test.sh` | Multi-bridge failover resilience |
| 5 | MEV/Censorship | `05_mev_censorship.ts` | MEV and censorship resistance metrics |

### Running Benchmarks

```bash
# Reset previous benchmark data
bash scripts/benchmark/00_clean_outputs.sh

# Grant execute permissions
chmod +x scripts/benchmark/*.sh

# Run smoke benchmarks (fast, ~1-2 min each)
bash scripts/benchmark/01_offchain_compute.sh --quick
bash scripts/benchmark/03_latency_breakdown.sh --quick
bash scripts/benchmark/04_failover_test.sh --quick

# Run on-chain benchmarks (requires Hardhat node)
cd contracts
npx hardhat run ../scripts/benchmark/02_gas_metrics.ts --network hardhat
npx hardhat run ../scripts/benchmark/05_mev_censorship.ts --network hardhat
```

### Generating Plots

```bash
python3 scripts/plot/plot_offchain_compute.py
python3 scripts/plot/plot_gas_metrics.py
python3 scripts/plot/plot_latency_breakdown.py
python3 scripts/plot/plot_failover.py
python3 scripts/plot/plot_mev_censorship.py
```

Plots are saved to `scripts/benchmark/data/charts/` as 300 DPI PNGs with IEEE serif fonts.

For full details, see [scripts/README_benchmark.md](scripts/README_benchmark.md).

---

## MPC Cluster (Docker)

Spin up a local 3-of-4 MPC committee for stress testing:

```bash
cd docker
VDF_T=262144 ZK_PROVER_ENABLED=true docker-compose up -d

# Check status
docker-compose ps
docker-compose logs -f node-1
```

---

## Key Design Decisions

| Decision | Rationale |
|---|---|
| **Halo2/IPA native prover** instead of a heavy zkVM | No trusted setup ceremony required. Runs as Rust-native code with practical proving times on commodity hardware. |
| **IQCG VDF** instead of RSA-based VDF | RSA requires a trusted setup to generate a modulus with unknown factorization. IQCG dynamically derives a class group from the input seed, making it fully decentralized and setup-free. |
| **Bridge adapter pattern** | Cross-chain protocols frequently change APIs. By abstracting behind `IBridgeAdapter`, the transport layer can be swapped (e.g., Axelar → CCIP) without modifying or re-auditing the core router logic. |
| **Optimistic verification** | Full VDF verification on-chain (via `0x05` modexp) can cost 500k+ gas. Optimistic mode reduces the happy-path cost to ~116k gas (~62% savings). The expensive verification only runs when a result is challenged. |
| **Separate crypto_engine & network_module** | Clean separation of concerns: pure cryptography has no knowledge of blockchains or I/O. This makes the crypto layer independently testable, auditable, and reusable. |

---

## Future Work

- **Decentralized MPC TSS** — Move from simulated single-node MPC to a real distributed threshold signing ceremony
- **Full BLS12-381 pairing circuit** — Tier-2 circuit with complete pairing verification (~16M constraints)
- **Frontend DApp + subgraph indexer** — User-facing interface for requesting and consuming randomness
- **Hardware-accelerated VDF** — ASIC/FPGA implementations for faster sequential squaring
- **Production bridge integrations** — Mainnet deployments with real Axelar/LayerZero/Wormhole relayers

---

## License & Acknowledgments

Academic research prototype.

Built on: [Halo2](https://github.com/zcash/halo2) · [vdf-rs](https://github.com/poanetwork/vdf) · [bls-signatures](https://github.com/filecoin-project/bls-signatures) · [Hardhat](https://hardhat.org/) · [Axelar](https://axelar.network/) · [LayerZero](https://layerzero.network/) · [Wormhole](https://wormhole.com/)
