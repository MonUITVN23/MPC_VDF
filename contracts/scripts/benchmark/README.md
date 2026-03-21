# Benchmark Automation Scripts

## 1) Testnet E2E User Cronjob (Sepolia)

Script: `request_cronjob.ts`

Mục tiêu: gửi định kỳ `requestRandomness(userSeed)` để Off-chain router xử lý và ghi `e2e_metrics.csv`.

Biến môi trường chính:
- `SEPOLIA_RPC_URL`
- `PRIVATE_KEY`
- `RANDOM_SENDER_ADDRESS`
- `REQUEST_INTERVAL_SECONDS` (mặc định `15`)
- `REQUEST_MAX_RETRIES` (mặc định `5`)
- `REQUEST_RETRY_DELAY_SECS` (mặc định `8`)
- `REQUEST_MAX_REQUESTS` (mặc định `0`, tức chạy vô hạn)

Ví dụ giới hạn an toàn (15s/lần, dừng sau 150 requests):

```bash
REQUEST_INTERVAL_SECONDS=15 REQUEST_MAX_REQUESTS=150 ts-node scripts/benchmark/request_cronjob.ts
```

Chạy:

```bash
cd /home/xuananh/mpc-vdf/contracts
ts-node scripts/benchmark/request_cronjob.ts
```

## 2) Local Stress Test (Crypto + Gas)

Script: `off-chain/network_module/src/bin/local_stress_benchmark.rs`

Mục tiêu: chạy local pipeline MPC→VDF và đo gas verify trên local Anvil, ghi `crypto_benchmarks.csv`.

Biến môi trường chính:
- `BENCH_T_VALUES` (mặc định `32768,1048576,4194304`)
- `BENCH_REPEATS_PER_T` (mặc định `5`)
- `CRYPTO_BENCH_CSV_PATH` (mặc định `crypto_benchmarks.csv`)
- `VDF_MODULUS_HEX` (tùy chọn)
- `VDF_MOCK_ARTIFACT_PATH` (tùy chọn)

Chạy:

```bash
cd /home/xuananh/mpc-vdf/off-chain
cargo run -p network_module --bin local_stress_benchmark
```

### Hardhat local + Rust VDF (khuyến nghị cho paper)

Script: `scripts/benchmark/local_crypto_stress.ts`

Script này:
- gọi Rust (`vdf_pipeline_once`) để lấy `prover_time_ms` và proof,
- deploy `VDFVerifierMock` trên Hardhat local,
- gửi tx verify để lấy `verify_gas_used`,
- ghi `off-chain/crypto_benchmarks.csv` với cột:
	`T_value,prover_time_ms,verify_gas_used`

Mặc định T sweep: `2^15, 2^18, 2^20`.

Chạy:

```bash
cd /home/xuananh/mpc-vdf/contracts
npm run bench:local-crypto
```

Tùy chỉnh:

```bash
cd /home/xuananh/mpc-vdf/contracts
BENCH_T_VALUES=32768,262144,1048576 BENCH_REPEATS_PER_T=20 npm run bench:local-crypto
```

## 2.1) Tối ưu phí Axelar trong Rust Node

Trong `off-chain/network_module/src/main.rs`, default `cross_chain_fee_wei` đã đặt về:
- `200000000000000` wei (`0.0002 ETH`)

Khuyến nghị set trong `.env` để chủ động:

```bash
CROSS_CHAIN_FEE_WEI=200000000000000
```

Nếu chưa dùng `CROSS_CHAIN_FEE_WEI`, code vẫn fallback từ `AXELAR_NATIVE_GAS_FEE_WEI`.

## 3) Post-processing Summary (JSON cho biểu đồ)

Script: `postprocess_metrics.ts`

Đọc đồng thời:
- `off-chain/crypto_benchmarks.csv`
- `off-chain/e2e_metrics.csv`

Sinh ra:
- `off-chain/crypto_summary.json`
- `off-chain/e2e_summary.json`

Biến môi trường tùy chọn:
- `CRYPTO_CSV_PATH`
- `E2E_CSV_PATH`
- `SUMMARY_OUTPUT_DIR`

Chạy:

```bash
cd /home/xuananh/mpc-vdf/contracts
npm run postprocess:metrics
```